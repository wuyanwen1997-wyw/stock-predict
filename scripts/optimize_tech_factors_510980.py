#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Offline search technical rules for 510980 next-day direction."""

from __future__ import annotations

import json
import urllib.request
from dataclasses import dataclass

UA = {"User-Agent": "Mozilla/5.0"}


def get(url: str) -> str:
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=20) as r:
        return r.read().decode("utf-8", "replace")


def fetch_klines(n: int = 280) -> list[dict]:
    url = f"https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sh510980,day,,,{n},qfq"
    kd = json.loads(get(url))
    rows = ((kd.get("data") or {}).get("sh510980") or {}).get("qfqday") or (
        ((kd.get("data") or {}).get("sh510980") or {}).get("day") or []
    )
    out = []
    for r in rows:
        out.append(
            {
                "date": r[0],
                "open": float(r[1]),
                "close": float(r[2]),
                "high": float(r[3]),
                "low": float(r[4]),
                "volume": float(r[5]),
            }
        )
    return out


def sma(closes: list[float], i: int, p: int) -> float | None:
    if i + 1 < p:
        return None
    return sum(closes[i + 1 - p : i + 1]) / p


def rsi(closes: list[float], i: int, p: int = 14) -> float | None:
    if i < p:
        return None
    gains = losses = 0.0
    for j in range(i - p + 1, i + 1):
        d = closes[j] - closes[j - 1]
        if d > 0:
            gains += d
        else:
            losses -= d
    if losses < 1e-12:
        return 100.0
    rs = gains / losses
    return 100.0 - 100.0 / (1.0 + rs)


def mom(closes: list[float], i: int, p: int) -> float | None:
    if i < p:
        return None
    prev = closes[i - p]
    if prev <= 0:
        return None
    return (closes[i] - prev) / prev


def vol_ratio(vols: list[float], i: int, p: int = 20) -> float | None:
    if i + 1 < p:
        return None
    avg = sum(vols[i + 1 - p : i + 1]) / p
    if avg <= 0:
        return 1.0
    return vols[i] / avg


@dataclass
class Feat:
    date: str
    close: float
    label: int  # next day
    ma5: float
    ma10: float
    ma20: float
    rsi14: float
    m1: float
    m5: float
    m10: float
    vr: float
    ma_dev: float
    yret: float


def build_features(klines: list[dict]) -> list[Feat]:
    closes = [k["close"] for k in klines]
    vols = [k["volume"] for k in klines]
    feats = []
    for i in range(len(klines) - 1):
        ma5 = sma(closes, i, 5)
        ma10 = sma(closes, i, 10)
        ma20 = sma(closes, i, 20)
        r = rsi(closes, i, 14)
        m1 = mom(closes, i, 1)
        m5 = mom(closes, i, 5)
        m10 = mom(closes, i, 10)
        vr = vol_ratio(vols, i, 20)
        if None in (ma5, ma10, ma20, r, m1, m5, m10, vr):
            continue
        label = 1 if closes[i + 1] > closes[i] else -1
        yret = m1 if m1 is not None else 0.0
        feats.append(
            Feat(
                date=klines[i]["date"],
                close=closes[i],
                label=label,
                ma5=ma5,
                ma10=ma10,
                ma20=ma20,
                rsi14=r,
                m1=yret,
                m5=m5,
                m10=m10,
                vr=vr,
                ma_dev=(closes[i] - ma20) / ma20,
                yret=yret,
            )
        )
    return feats


def acc(preds: list[int], labels: list[int]) -> tuple[float, int, int]:
    ok = sum(1 for p, l in zip(preds, labels) if p == l)
    n = len(labels)
    return (ok / n if n else 0.0), ok, n


def current_style_score(f: Feat) -> float:
    """Mimic existing score_factors roughly."""
    score = 0.0
    price = f.close
    if price > f.ma5 > f.ma10 > f.ma20:
        score += 1.0
    elif price < f.ma5 < f.ma10 < f.ma20:
        score -= 1.0
    else:
        score += 0.25 if price > f.ma20 else -0.25
    if f.ma_dev > 0.06:
        score -= 0.4
    elif f.ma_dev < -0.06:
        score += 0.4
    elif f.ma_dev > 0:
        score += 0.2
    else:
        score -= 0.2
    if f.rsi14 < 32:
        score += 0.7
    elif f.rsi14 > 68:
        score -= 0.7
    elif f.rsi14 > 55:
        score += 0.25
    elif f.rsi14 < 45:
        score -= 0.25
    score += max(-0.06, min(0.06, f.m5)) * 10
    score += max(-0.08, min(0.08, f.m10)) * 5
    if f.vr > 1.4:
        if score > 0:
            score += 0.25
        elif score < 0:
            score -= 0.25
    elif f.vr < 0.7:
        score *= 0.85
    return max(-2.5, min(2.5, score))


def main() -> None:
    klines = fetch_klines(280)
    feats = build_features(klines)
    # last 120 like UI
    sample = feats[-120:]
    labels = [f.label for f in sample]
    print(f"klines={len(klines)} feats={len(feats)} sample={len(sample)}", flush=True)
    base_up = sum(1 for l in labels if l > 0) / len(labels)
    print(f"baseline always_up={base_up:.1%}", flush=True)

    # current model
    preds = [1 if current_style_score(f) >= 0 else -1 for f in sample]
    a, ok, n = acc(preds, labels)
    print(f"current_model acc={a:.1%} ({ok}/{n})", flush=True)

    results = []

    # single factor edges
    rules = []
    for name, fn in [
        ("fade_1d", lambda f: -1 if f.yret > 0 else 1),
        ("follow_1d", lambda f: 1 if f.yret > 0 else -1),
        ("fade_5d", lambda f: -1 if f.m5 > 0 else 1),
        ("follow_5d", lambda f: 1 if f.m5 > 0 else -1),
        ("rsi_mr_30_70", lambda f: 1 if f.rsi14 < 30 else (-1 if f.rsi14 > 70 else (1 if f.yret < 0 else -1))),
        ("rsi_mr_35_65", lambda f: 1 if f.rsi14 < 35 else (-1 if f.rsi14 > 65 else (1 if f.yret < 0 else -1))),
        ("below_ma20_buy", lambda f: 1 if f.close < f.ma20 else -1),
        ("above_ma20_buy", lambda f: 1 if f.close > f.ma20 else -1),
        ("ma_bull", lambda f: 1 if f.close > f.ma5 > f.ma10 else -1),
        ("ma_bear_fade", lambda f: 1 if f.close < f.ma5 < f.ma10 else -1),  # buy dips
    ]:
        preds = [fn(f) for f in sample]
        a, ok, n = acc(preds, labels)
        rules.append((a, name, ok, n))
    rules.sort(reverse=True)
    print("\n=== single rules ===", flush=True)
    for a, name, ok, n in rules[:15]:
        print(f"{name}: {a:.1%} ({ok}/{n})", flush=True)

    # grid: mean-reversion blend
    for rsi_lo, rsi_hi in [(30, 70), (35, 65), (40, 60)]:
        for m1_w in [0.5, 1.0, 1.5, 2.0]:
            for ma_w in [0.0, 0.3, 0.6]:
                for mom_sign in [-1.0, 0.0]:  # -1 = fade momentum
                    preds = []
                    for f in sample:
                        s = 0.0
                        # fade yesterday
                        s += (-f.yret) * m1_w * 40
                        # RSI MR
                        if f.rsi14 < rsi_lo:
                            s += 1.0
                        elif f.rsi14 > rsi_hi:
                            s -= 1.0
                        else:
                            s += (50 - f.rsi14) / 50 * 0.5
                        # MA deviation MR
                        s += (-f.ma_dev) * ma_w * 8
                        # momentum
                        s += f.m5 * mom_sign * 15
                        preds.append(1 if s >= 0 else -1)
                    a, ok, n = acc(preds, labels)
                    results.append(
                        {
                            "acc": round(a * 100, 1),
                            "ok": ok,
                            "n": n,
                            "rsi": (rsi_lo, rsi_hi),
                            "m1_w": m1_w,
                            "ma_w": ma_w,
                            "mom_sign": mom_sign,
                        }
                    )

    results.sort(key=lambda x: -x["acc"])
    print("\n=== TOP mean-reversion blends ===", flush=True)
    for r in results[:20]:
        print(r, flush=True)

    # holdout: train on first half of sample, test on second
    mid = len(sample) // 2
    train, test = sample[:mid], sample[mid:]
    best = None
    for rsi_lo, rsi_hi in [(30, 70), (35, 65), (40, 60)]:
        for m1_w in [0.8, 1.2, 1.6, 2.0]:
            for ma_w in [0.2, 0.5, 0.8]:
                def score_fn(f, rsi_lo=rsi_lo, rsi_hi=rsi_hi, m1_w=m1_w, ma_w=ma_w):
                    s = (-f.yret) * m1_w * 40
                    if f.rsi14 < rsi_lo:
                        s += 1.0
                    elif f.rsi14 > rsi_hi:
                        s -= 1.0
                    else:
                        s += (50 - f.rsi14) / 50 * 0.6
                    s += (-f.ma_dev) * ma_w * 8
                    s += (-f.m5) * 8  # fade 5d
                    return s

                tr_preds = [1 if score_fn(f) >= 0 else -1 for f in train]
                te_preds = [1 if score_fn(f) >= 0 else -1 for f in test]
                tr_a, _, _ = acc(tr_preds, [f.label for f in train])
                te_a, te_ok, te_n = acc(te_preds, [f.label for f in test])
                cand = {
                    "train": round(tr_a * 100, 1),
                    "test": round(te_a * 100, 1),
                    "te_ok": te_ok,
                    "te_n": te_n,
                    "rsi": (rsi_lo, rsi_hi),
                    "m1_w": m1_w,
                    "ma_w": ma_w,
                }
                if best is None or cand["test"] > best["test"]:
                    best = cand
    print("\n=== best holdout ===", flush=True)
    print(best, flush=True)

    # evaluate best-looking simple: fade_1d + rsi
    def best_score(f: Feat) -> float:
        s = (-f.yret) * 1.5 * 40
        if f.rsi14 < 35:
            s += 1.0
        elif f.rsi14 > 65:
            s -= 1.0
        else:
            s += (50 - f.rsi14) / 50 * 0.6
        s += (-f.ma_dev) * 0.5 * 8
        s += (-f.m5) * 8
        return s

    preds = [1 if best_score(f) >= 0 else -1 for f in sample]
    a, ok, n = acc(preds, labels)
    print(f"\nproposed full120: {a:.1%} ({ok}/{n})", flush=True)

    # high conf if |score| large
    hc_ok = hc_n = 0
    for f in sample:
        s = best_score(f)
        if abs(s) < 0.8:
            continue
        hc_n += 1
        if (1 if s >= 0 else -1) == f.label:
            hc_ok += 1
    print(f"proposed hc: {hc_ok/max(hc_n,1):.1%} ({hc_ok}/{hc_n})", flush=True)


if __name__ == "__main__":
    main()
