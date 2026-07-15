#!/usr/bin/env python3
"""训练 LightGBM 方向预测模型并导出 ONNX（与 Rust features.rs 对齐）"""

from __future__ import annotations

import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
STOCKS_JSON = ROOT / "src-tauri" / "resources" / "stocks.json"
OUT_DIR = ROOT / "src-tauri" / "resources" / "models"
OUT_ONNX = OUT_DIR / "lgbm_v1.onnx"
OUT_META = OUT_DIR / "lgbm_meta.json"

FEATURE_NAMES = [
    "ret_1", "ret_5", "ret_10",
    "ma5_ratio", "ma10_ratio", "ma20_ratio", "ma5_ma20",
    "rsi14", "volume_ratio", "volatility",
    "range_pct", "close_pos",
    "macd_hist", "kdj_j", "boll_pct", "ma_align",
]
MIN_BARS = 35
FLAT_THRESHOLD = 0.003
TENCENT_URL = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get"


def fetch_klines(market: str, code: str, limit: int = 320) -> list[dict]:
    symbol = f"sh{code}" if market == "SH" else f"sz{code}"
    param = f"{symbol},day,,,{limit},qfq"
    url = f"{TENCENT_URL}?{urllib.parse.urlencode({'param': param})}"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0", "Referer": "https://gu.qq.com/"})
    with urllib.request.urlopen(req, timeout=15) as resp:
        data = json.loads(resp.read().decode())

    rows = data.get("data", {}).get(symbol, {}).get("qfqday", [])
    bars = []
    for row in rows:
        if len(row) < 6:
            continue
        bars.append({
            "date": row[0],
            "open": float(row[1]),
            "close": float(row[2]),
            "high": float(row[3]),
            "low": float(row[4]),
            "volume": float(row[5]),
        })
    return bars


def sma(values: list[float], period: int) -> float | None:
    if len(values) < period:
        return None
    return sum(values[-period:]) / period


def return_over(closes: list[float], period: int) -> float | None:
    if len(closes) <= period:
        return None
    prev = closes[-1 - period]
    if prev <= 0:
        return None
    return (closes[-1] - prev) / prev


def volume_ratio(bars: list[dict], period: int) -> float:
    vols = [b["volume"] for b in bars[-period:]]
    avg = sum(vols) / len(vols)
    if avg <= 0:
        return 1.0
    return bars[-1]["volume"] / avg


def rsi(closes: list[float], period: int) -> float | None:
    if len(closes) <= period:
        return None
    gains = losses = 0.0
    for i in range(len(closes) - period, len(closes)):
        delta = closes[i] - closes[i - 1]
        if delta > 0:
            gains += delta
        else:
            losses -= delta
    if losses < 1e-9:
        return 100.0
    rs = gains / losses
    return 100.0 - 100.0 / (1.0 + rs)


def calc_volatility(closes: list[float], window: int = 20) -> float:
    if len(closes) < window + 1:
        return 0.02
    rets = []
    for i in range(len(closes) - window, len(closes)):
        prev = closes[i - 1]
        if prev > 0:
            rets.append((closes[i] - prev) / prev)
    if not rets:
        return 0.02
    mean = sum(rets) / len(rets)
    var = sum((r - mean) ** 2 for r in rets) / len(rets)
    return max(0.005, min(0.08, var ** 0.5))


def ema_series(values: list[float], period: int) -> list[float]:
    alpha = 2.0 / (period + 1.0)
    seed = sum(values[:period]) / period
    out = [seed] * period
    prev = seed
    for v in values[period:]:
        prev = alpha * v + (1 - alpha) * prev
        out.append(prev)
    return out


def macd_hist_norm(closes: list[float], close: float) -> float:
    if len(closes) < 26:
        return 0.0
    ema12 = ema_series(closes, 12)
    ema26 = ema_series(closes, 26)
    macd_line = [a - b for a, b in zip(ema12, ema26)]
    signal_len = min(9, len(macd_line))
    alpha = 2.0 / 10.0
    seed = sum(macd_line[-signal_len:]) / signal_len
    signal = seed
    for v in macd_line[-signal_len:]:
        signal = alpha * v + (1 - alpha) * signal
    hist = macd_line[-1] - signal
    return hist / close if close > 0 else 0.0


def kdj_j(bars: list[dict], period: int = 9) -> float:
    window = bars[-period:]
    low = min(b["low"] for b in window)
    high = max(b["high"] for b in window)
    close = bars[-1]["close"]
    if high - low < 1e-9:
        return 50.0
    rsv = (close - low) / (high - low) * 100.0
    k = 0.667 * 50.0 + 0.333 * rsv
    d = 0.667 * 50.0 + 0.333 * k
    return 3.0 * k - 2.0 * d


def boll_pct(closes: list[float], period: int, close: float) -> float:
    if len(closes) < period:
        return 0.5
    slice_ = closes[-period:]
    mean = sum(slice_) / period
    var = sum((c - mean) ** 2 for c in slice_) / period
    std = var ** 0.5
    if std < 1e-9:
        return 0.5
    upper = mean + 2 * std
    lower = mean - 2 * std
    return max(0.0, min(1.0, (close - lower) / (upper - lower)))


def ma_align(close: float, ma5: float, ma10: float, ma20: float) -> float:
    score = 0.0
    score += 1 if close > ma5 else -1
    score += 1 if ma5 > ma10 else -1
    score += 1 if ma10 > ma20 else -1
    return score / 3.0


def extract_features(bars: list[dict]) -> list[float] | None:
    if len(bars) < MIN_BARS:
        return None
    closes = [b["close"] for b in bars]
    last = bars[-1]
    close = last["close"]
    if close <= 0:
        return None

    ma5 = sma(closes, 5)
    ma10 = sma(closes, 10)
    ma20 = sma(closes, 20)
    if ma5 is None or ma10 is None or ma20 is None:
        return None

    ret1 = return_over(closes, 1)
    ret5 = return_over(closes, 5)
    ret10 = return_over(closes, 10)
    rsi14 = rsi(closes, 14)
    if ret1 is None or ret5 is None or ret10 is None or rsi14 is None:
        return None

    vol_ratio = volume_ratio(bars, 20)
    vol = calc_volatility(closes)
    range_pct = (last["high"] - last["low"]) / close
    close_pos = 0.5 if abs(last["high"] - last["low"]) < 1e-9 else (close - last["low"]) / (last["high"] - last["low"])

    return [
        ret1, ret5, ret10,
        close / ma5 - 1, close / ma10 - 1, close / ma20 - 1, ma5 / ma20 - 1,
        rsi14 / 100.0, vol_ratio, vol,
        range_pct, close_pos,
        macd_hist_norm(closes, close), kdj_j(bars) / 100.0, boll_pct(closes, 20, close), ma_align(close, ma5, ma10, ma20),
    ]


def label_next(bars: list[dict], idx: int) -> int | None:
    if idx + 1 >= len(bars):
        return None
    prev = bars[idx]["close"]
    nxt = bars[idx + 1]["close"]
    if prev <= 0:
        return None
    chg = (nxt - prev) / prev
    if chg > FLAT_THRESHOLD:
        return 2  # up
    if chg < -FLAT_THRESHOLD:
        return 0  # down
    return 1  # flat


def build_dataset(stocks: list[dict]) -> tuple[np.ndarray, np.ndarray]:
    xs, ys = [], []
    for stock in stocks:
        code = stock["code"]
        market = stock["market"]
        print(f"  拉取 {stock['name']} ({code})...")
        try:
            bars = fetch_klines(market, code)
        except Exception as exc:
            print(f"    跳过: {exc}")
            continue
        for i in range(MIN_BARS - 1, len(bars) - 1):
            feats = extract_features(bars[: i + 1])
            label = label_next(bars, i)
            if feats is None or label is None:
                continue
            xs.append(feats)
            ys.append(label)
    return np.array(xs, dtype=np.float32), np.array(ys, dtype=np.int64)


def main() -> int:
    try:
        import lightgbm as lgb
        from sklearn.metrics import accuracy_score, classification_report
        from sklearn.model_selection import train_test_split
        from skl2onnx import convert_sklearn
        from skl2onnx.common.data_types import FloatTensorType
    except ImportError:
        print("请先安装依赖: pip install -r scripts/requirements.txt")
        return 1

    stocks = json.loads(STOCKS_JSON.read_text(encoding="utf-8"))
    print(f"从 {len(stocks)} 只股票构建训练集...")
    X, y = build_dataset(stocks)
    if len(X) < 200:
        print(f"样本不足 ({len(X)} 条)，至少需要 200 条")
        return 1

    print(f"总样本: {len(X)}")
    X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, shuffle=False)

    clf = lgb.LGBMClassifier(
        objective="multiclass",
        num_class=3,
        n_estimators=200,
        learning_rate=0.05,
        max_depth=6,
        num_leaves=31,
        subsample=0.8,
        colsample_bytree=0.8,
        random_state=42,
        verbose=-1,
    )
    clf.fit(X_train, y_train)

    pred = clf.predict(X_test)
    acc = accuracy_score(y_test, pred)
    print(f"测试集准确率: {acc:.3f}")
    print(classification_report(y_test, pred, target_names=["down", "flat", "up"]))

    initial_type = [("float_input", FloatTensorType([None, len(FEATURE_NAMES)]))]
    options = {id(clf): {"zipmap": False}}
    onnx_model = convert_sklearn(
        clf,
        initial_types=initial_type,
        options=options,
        target_opset=12,
    )

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with open(OUT_ONNX, "wb") as f:
        f.write(onnx_model.SerializeToString())

    meta = {
        "feature_names": FEATURE_NAMES,
        "class_order": ["down", "flat", "up"],
        "class_ids": [0, 1, 2],
        "flat_threshold": FLAT_THRESHOLD,
        "test_accuracy": round(float(acc), 4),
        "samples": int(len(X)),
    }
    OUT_META.write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding="utf-8")

    print(f"已导出: {OUT_ONNX}")
    print(f"元数据: {OUT_META}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
