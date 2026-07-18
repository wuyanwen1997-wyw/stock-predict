"""Offline probe: 宽基主力资金 / 北向净流入 vs 510980 next-day."""
from __future__ import annotations

import time
from collections import defaultdict

import requests

H = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    "Referer": "https://data.eastmoney.com/",
}


def get_json(url: str, params: dict | None = None, retries: int = 4) -> dict:
    last = None
    for i in range(retries):
        try:
            r = requests.get(url, params=params, headers=H, timeout=25)
            r.raise_for_status()
            return r.json()
        except Exception as e:
            last = e
            time.sleep(0.6 * (i + 1))
    raise RuntimeError(last)


def fetch_index_main_flow(secid: str = "1.000001", lmt: int = 400) -> dict[str, float]:
    out: dict[str, float] = {}
    # daykline 往往一次返回较长历史；push2delay 作兜底
    endpoints = [
        (
            "https://push2.eastmoney.com/api/qt/stock/fflow/daykline/get",
            {
                "lmt": 0,
                "klt": 101,
                "secid": secid,
                "fields1": "f1,f2,f3,f7",
                "fields2": "f51,f52,f53,f54,f55,f56",
                "ut": "b2884a393a59ad64002292a3e90d46a5",
            },
        ),
        (
            "https://push2delay.eastmoney.com/api/qt/stock/fflow/kline/get",
            {
                "secid": secid,
                "fields1": "f1,f2,f3,f7",
                "fields2": "f51,f52,f53,f54,f55,f56",
                "klt": 101,
                "lmt": lmt,
            },
        ),
    ]
    for url, params in endpoints:
        try:
            j = get_json(url, params)
            for line in (j.get("data") or {}).get("klines") or []:
                p = line.split(",")
                out[p[0]] = float(p[1])  # 主力净流入（元）
            if len(out) > 20:
                return out
        except Exception as e:
            print("fflow try fail", url.split("/")[2], e)
            time.sleep(0.5)
    return out


def fetch_north_net(pages: int = 25) -> dict[str, float]:
    net: dict[str, float] = defaultdict(float)
    for page in range(1, pages + 1):
        for typ in ("001", "003"):  # 沪股通 / 深股通
            j = get_json(
                "https://datacenter-web.eastmoney.com/api/data/v1/get",
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
                },
            )
            for r in (j.get("result") or {}).get("data") or []:
                d = (r.get("TRADE_DATE") or "")[:10]
                if r.get("NET_DEAL_AMT") is not None:
                    net[d] += float(r["NET_DEAL_AMT"])  # 亿元
        time.sleep(0.05)
    return dict(net)


def fetch_510980():
    j = get_json(
        "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get",
        {"param": "sh510980,day,,,400,qfq"},
    )
    rows = ((j.get("data") or {}).get("sh510980") or {}).get("qfqday") or (
        ((j.get("data") or {}).get("sh510980") or {}).get("day") or []
    )
    return [
        (r[0], float(r[2]), float(r[3]), float(r[4]), float(r[5])) for r in rows
    ]  # d,c,h,l,v


def sma(xs, i, p):
    return sum(xs[j] for j in range(i + 1 - p, i + 1)) / p


def main():
    sh = fetch_index_main_flow()
    print("index main flow days", len(sh), "last", sorted(sh.items())[-2:])
    north = fetch_north_net(20)
    print("north net days", len(north), "max", max(north) if north else None)

    bars = fetch_510980()
    by = {d: (c, h, l, v) for d, c, h, l, v in bars}
    dates = [d for d, *_ in bars if d in sh]
    closes = [by[d][0] for d in dates]
    highs = [by[d][1] for d in dates]
    lows = [by[d][2] for d in dates]
    vols = [by[d][3] for d in dates]
    flows = [sh[d] for d in dates]

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

        window = flows[i - 19 : i + 1]
        scale = sorted(abs(x) for x in window)[len(window) // 2] or 1.0
        fn = flows[i] / scale
        f5 = sum(flows[i - 4 : i + 1]) / scale
        flow = max(-2.5, min(2.5, fn * 0.6 + f5 * 0.4))
        flow_fade = -flow
        lab = 1 if closes[i + 1] > c else -1
        feats.append({"f": f, "flow": flow, "fade": flow_fade, "lab": lab, "fn": fn})

    s = feats[-120:]
    print("eval window", len(s), "feat total", len(feats))
    if not s:
        print("insufficient overlap; abort")
        return

    def acc(key):
        ok = sum((1 if x[key] >= 0 else -1) == x["lab"] for x in s)
        return ok / len(s)

    print(f"factor={acc('f'):.1%} flow={acc('flow'):.1%} fade={acc('fade'):.1%}")
    for w in (0.1, 0.15, 0.2, 0.25, 0.3):
        a = sum(
            (1 if (1 - w) * x["f"] + w * x["flow"] >= 0 else -1) == x["lab"] for x in s
        ) / len(s)
        b = sum(
            (1 if (1 - w) * x["f"] + w * x["fade"] >= 0 else -1) == x["lab"] for x in s
        ) / len(s)
        print(f"factor+flow@{w}: {a:.1%}  factor+fade@{w}: {b:.1%}")

    # north alone on overlapping historical (not necessarily last 120)
    ds = sorted(set(north) & set(by))
    pairs = []
    for i, d in enumerate(ds[:-1]):
        d2 = ds[i + 1]
        lab = 1 if by[d2][0] > by[d][0] else -1
        pairs.append((north[d], lab))
    if pairs:
        t = pairs[-250:]
        a = sum((1 if m >= 0 else -1) == lab for m, lab in t) / len(t)
        print(f"north_net historical last{len(t)}: {a:.1%}")


if __name__ == "__main__":
    main()
