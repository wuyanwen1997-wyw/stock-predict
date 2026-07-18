"""Validate 北向净流入 (MUTUAL_TYPE=005 / 001+003) vs 510980 + factor."""
from __future__ import annotations

import time
from collections import defaultdict

import requests

H = {
    "User-Agent": "Mozilla/5.0",
    "Referer": "https://data.eastmoney.com/hsgt/index.html",
}


def get(params, retries=3):
    last = None
    for i in range(retries):
        try:
            r = requests.get(
                "https://datacenter-web.eastmoney.com/api/data/v1/get",
                params=params,
                headers=H,
                timeout=20,
            )
            return r.json()
        except Exception as e:
            last = e
            time.sleep(0.4 * (i + 1))
    raise RuntimeError(last)


def fetch_north(typ: str, pages: int = 30) -> dict[str, float]:
    out: dict[str, float] = {}
    for page in range(1, pages + 1):
        j = get(
            {
                "sortColumns": "TRADE_DATE",
                "sortTypes": -1,
                "pageSize": 50,
                "pageNumber": page,
                "reportName": "RPT_MUTUAL_DEAL_HISTORY",
                "columns": "ALL",
                "source": "WEB",
                "client": "WEB",
                "filter": f'(MUTUAL_TYPE="{typ}")',
            }
        )
        rows = (j.get("result") or {}).get("data") or []
        if not rows:
            break
        for r in rows:
            d = (r.get("TRADE_DATE") or "")[:10]
            if r.get("NET_DEAL_AMT") is not None:
                out[d] = float(r["NET_DEAL_AMT"])
        time.sleep(0.05)
        if (rows[-1].get("TRADE_DATE") or "")[:10] < "2020-01-01":
            break
    return out


def fetch_510980():
    j = requests.get(
        "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get",
        params={"param": "sh510980,day,,,900,qfq"},
        headers=H,
        timeout=20,
    ).json()
    rows = ((j.get("data") or {}).get("sh510980") or {}).get("qfqday") or (
        ((j.get("data") or {}).get("sh510980") or {}).get("day") or []
    )
    return [(r[0], float(r[2]), float(r[3]), float(r[4]), float(r[5])) for r in rows]


def sma(xs, i, p):
    return sum(xs[j] for j in range(i + 1 - p, i + 1)) / p


def main():
    n5 = fetch_north("005", 40)
    n1 = fetch_north("001", 40)
    n3 = fetch_north("003", 40)
    north = dict(n5)
    # fill gaps with 001+003
    for d in set(n1) | set(n3):
        if d not in north:
            if d in n1 or d in n3:
                north[d] = n1.get(d, 0.0) + n3.get(d, 0.0)
    print("north days", len(north), "max", max(north) if north else None, "min", min(north) if north else None)

    bars = fetch_510980()
    by = {d: (c, h, l, v) for d, c, h, l, v in bars}
    dates = [d for d, *_ in bars if d in north]
    print("overlap", len(dates), "range", dates[0] if dates else None, "->", dates[-1] if dates else None)

    closes = [by[d][0] for d in dates]
    highs = [by[d][1] for d in dates]
    lows = [by[d][2] for d in dates]
    vols = [by[d][3] for d in dates]
    nets = [north[d] for d in dates]  # 亿元

    feats = []
    for i in range(60, len(dates) - 1):
        c = closes[i]
        ma5, ma10, ma20 = sma(closes, i, 5), sma(closes, i, 10), sma(closes, i, 20)
        m1 = (c - closes[i - 1]) / closes[i - 1]
        above = 1 if c > ma20 else -1
        bull = 1 if c > ma5 > ma10 > ma20 else (-1 if c < ma5 < ma10 < ma20 else 0)
        fade = -1 if m1 > 0 else 1
        dev = (c - ma20) / ma20
        atr = 0.0
        for j in range(i - 13, i + 1):
            tr = max(
                highs[j] - lows[j],
                abs(highs[j] - closes[j - 1]),
                abs(lows[j] - closes[j - 1]),
            )
            atr += tr / closes[j]
        atr /= 14
        vr = vols[i] / (sum(vols[i - 19 : i + 1]) / 20)
        f = 0.5 * above + fade + 0.6 * bull - 3.0 * dev
        if atr > 0.015:
            f += 0.5 * fade
        if vr > 1.5 and m1 > 0:
            f -= 0.15

        # scale net by recent median abs (亿元)
        window = nets[i - 19 : i + 1]
        scale = sorted(abs(x) for x in window)[len(window) // 2] or 1.0
        n1s = nets[i] / scale
        n5s = sum(nets[i - 4 : i + 1]) / scale
        flow = max(-2.5, min(2.5, n1s * 0.55 + n5s * 0.45))
        fade_flow = -flow
        lab = 1 if closes[i + 1] > c else -1
        feats.append({"f": f, "flow": flow, "fade": fade_flow, "lab": lab, "date": dates[i]})

    print("feats", len(feats), "from", feats[0]["date"], "to", feats[-1]["date"])

    def eval_slice(s, tag):
        if not s:
            return
        def acc(key):
            return sum((1 if x[key] >= 0 else -1) == x["lab"] for x in s) / len(s)
        print(f"[{tag}] n={len(s)} factor={acc('f'):.1%} north={acc('flow'):.1%} fade={acc('fade'):.1%}")
        for w in (0.1, 0.15, 0.2, 0.25, 0.3):
            a = sum((1 if (1 - w) * x["f"] + w * x["flow"] >= 0 else -1) == x["lab"] for x in s) / len(s)
            b = sum((1 if (1 - w) * x["f"] + w * x["fade"] >= 0 else -1) == x["lab"] for x in s) / len(s)
            print(f"  +north@{w}:{a:.1%}  +fade@{w}:{b:.1%}")
        # agree-or-ignore
        ok = 0
        for x in s:
            sc = x["f"]
            if abs(x["flow"]) > 0.25 and (x["flow"] >= 0) == (x["f"] >= 0):
                sc = x["f"] + 0.35 * x["flow"]
            ok += (1 if sc >= 0 else -1) == x["lab"]
        print(f"  agree-or-ignore: {ok/len(s):.1%}")

    eval_slice(feats[-120:], "last120_with_net")
    eval_slice(feats[-250:], "last250_with_net")
    eval_slice(feats, "all")


if __name__ == "__main__":
    main()
