"""拉取大盘主力净流入历史，写入本地缓存供 App 回测使用。

优先级：
1. 环境变量 TUSHARE_TOKEN → moneyflow_mkt_dc
2. akshare.stock_market_fund_flow()（东财，部分网络不稳定）

输出：
- %LOCALAPPDATA%/stock-predict/market_fund_flow.json（Windows）
- 或 $STOCK_PREDICT_DATA/market_fund_flow.json
- 可选同步到 src-tauri/resources/market_fund_flow.json（--seed）
"""
from __future__ import annotations

import argparse
import json
import os
from datetime import datetime
from pathlib import Path


def data_dir() -> Path:
    if p := os.environ.get("STOCK_PREDICT_DATA"):
        return Path(p)
    local = os.environ.get("LOCALAPPDATA")
    if local:
        return Path(local) / "stock-predict"
    return Path.home() / ".stock-predict"


def fetch_tushare(token: str) -> dict[str, float]:
    import requests

    out: dict[str, float] = {}
    # 近约 800 个自然日
    for start, end in [("20240101", "20241231"), ("20250101", "20261231")]:
        body = {
            "api_name": "moneyflow_mkt_dc",
            "token": token,
            "params": {"start_date": start, "end_date": end},
            "fields": "trade_date,net_amount",
        }
        r = requests.post("https://api.tushare.pro", json=body, timeout=30)
        r.raise_for_status()
        j = r.json()
        data = j.get("data") or {}
        fields = data.get("fields") or ["trade_date", "net_amount"]
        items = data.get("items") or []
        if not items:
            msg = j.get("msg") or "empty"
            print(f"tushare {start}-{end}: {msg}")
            continue
        i_d = fields.index("trade_date") if "trade_date" in fields else 0
        i_n = fields.index("net_amount") if "net_amount" in fields else 1
        for row in items:
            d = str(row[i_d])
            if len(d) == 8:
                d = f"{d[:4]}-{d[4:6]}-{d[6:8]}"
            out[d] = float(row[i_n])
        print(f"tushare {start}-{end}: +{len(items)} (total {len(out)})")
    return out


def fetch_akshare() -> dict[str, float]:
    import akshare as ak

    df = ak.stock_market_fund_flow()
    # 列名可能是中文
    date_col = next(c for c in df.columns if "日期" in str(c) or c == "date")
    main_col = next(c for c in df.columns if "主力" in str(c) and "净流入" in str(c) and "占比" not in str(c))
    out: dict[str, float] = {}
    for _, row in df.iterrows():
        d = str(row[date_col])[:10]
        out[d] = float(row[main_col])
    print(f"akshare: {len(out)} days")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", action="store_true", help="同时写入 src-tauri/resources/")
    args = ap.parse_args()

    market: dict[str, float] = {}
    token = (os.environ.get("TUSHARE_TOKEN") or "").strip()
    token_file = data_dir() / "tushare_token.txt"
    if not token and token_file.exists():
        token = token_file.read_text(encoding="utf-8").strip()

    if token:
        try:
            market.update(fetch_tushare(token))
        except Exception as e:
            print("tushare failed:", e)
    else:
        print("no TUSHARE_TOKEN, try akshare…")

    if len(market) < 30:
        try:
            market.update(fetch_akshare())
        except Exception as e:
            print("akshare failed:", e)

    if not market:
        raise SystemExit("未能拉取任何大盘主力数据")

    payload = {
        "market_main": dict(sorted(market.items())),
        "north_net_yi": {},
        "updated_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
    }
    out_dir = data_dir()
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / "market_fund_flow.json"
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print("wrote", out_path, "days=", len(market))

    if args.seed:
        seed = Path(__file__).resolve().parents[1] / "src-tauri" / "resources" / "market_fund_flow.json"
        seed.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
        print("wrote", seed)


if __name__ == "__main__":
    main()
