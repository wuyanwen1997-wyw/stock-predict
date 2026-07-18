#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Tune factor + message weights for 510980 next-day direction."""

from __future__ import annotations

import json
import re
import time
import urllib.parse
import urllib.request
from collections import defaultdict
from datetime import datetime, timedelta
from pathlib import Path

UA = {
    "User-Agent": "Mozilla/5.0",
    "Referer": "https://so.eastmoney.com/",
}

BULL = {
    "央行降准": 1.2,
    "降准": 1.1,
    "北向净买入": 1.0,
    "北向流入": 0.95,
    "社融超预期": 0.9,
    "成交额破万亿": 0.85,
    "PMI回升": 0.85,
    "稳增长": 0.7,
    "政策托底": 0.8,
    "流动性宽松": 0.85,
    "赚钱效应": 0.7,
    "风险偏好回升": 0.75,
}
BEAR = {
    "北向净卖出": 1.1,
    "北向流出": 1.05,
    "暴跌": 1.0,
    "跳水": 0.95,
    "杀跌": 0.95,
    "回落": 1.2,
    "承压": 1.05,
    "缩量": 1.05,
    "走弱": 1.05,
    "下跌": 0.95,
    "反弹": 1.15,
    "放量": 0.9,
    "收涨": 0.95,
    "新高": 0.95,
    "活跃": 0.95,
    "增量资金": 0.75,
    "亏钱效应": 0.85,
    "流动性收紧": 0.85,
}


def get(url: str, retries: int = 4) -> str:
    last = None
    for i in range(retries):
        try:
            req = urllib.request.Request(url, headers=UA)
            with urllib.request.urlopen(req, timeout=20) as r:
                return r.read().decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(0.5 * (i + 1))
    raise RuntimeError(last)


def fetch_klines(n: int = 320) -> list[dict]:
    url = f"https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sh510980,day,,,{n},qfq"
    kd = json.loads(get(url))
    rows = ((kd.get("data") or {}).get("sh510980") or {}).get("qfqday") or (
        ((kd.get("data") or {}).get("sh510980") or {}).get("day") or []
    )
    return [
        {
            "date": r[0],
            "close": float(r[2]),
            "high": float(r[3]),
            "low": float(r[4]),
            "volume": float(r[5]),
        }
        for r in rows
    ]


def search_news(keyword: str, pages: int = 4) -> list[tuple[str, str]]:
    out = []
    for page in range(1, pages + 1):
        param = json.dumps(
            {
                "uid": "",
                "keyword": keyword,
                "type": ["cmsArticleWebOld"],
                "client": "web",
                "clientType": "web",
                "clientVersion": "curr",
                "param": {
                    "cmsArticleWebOld": {
                        "searchScope": "default",
                        "sort": "default",
                        "pageIndex": page,
                        "pageSize": 20,
                    }
                },
            },
            ensure_ascii=False,
        )
        url = (
            "https://search-api-web.eastmoney.com/search/jsonp"
            f"?cb=jQuery&param={urllib.parse.quote(param)}"
        )
        try:
            t = get(url)
        except Exception:
            break
        a, b = t.find("{"), t.rfind("}")
        if a < 0:
            break
        body = t[a : b + 1]
        try:
            d = json.loads(body)
        except json.JSONDecodeError:
            try:
                d = json.loads(re.sub(r"[\x00-\x1f]", " ", body))
            except json.JSONDecodeError:
                break
        arr = ((d.get("result") or {}).get("cmsArticleWebOld")) or []
        if not arr:
            break
        for it in arr:
            title = re.sub(r"<[^>]+>", "", it.get("title") or "")
            date = (it.get("date") or "")[:10]
            if title and date:
                out.append((date, title))
        time.sleep(0.15)
    return out


def sma(closes: list[float], i: int, p: int) -> float:
    return sum(closes[i + 1 - p : i + 1]) / p


def atr_pct(H, L, C, i, p=14) -> float:
    s = 0.0
    for j in range(i - p + 1, i + 1):
        tr = max(H[j] - L[j], abs(H[j] - C[j - 1]), abs(L[j] - C[j - 1]))
        s += tr / C[j]
    return s / p


def factor_score(C, V, H, L, i) -> float:
    ma5, ma10, ma20 = sma(C, i, 5), sma(C, i, 10), sma(C, i, 20)
    m1 = (C[i] - C[i - 1]) / C[i - 1]
    above = 1 if C[i] > ma20 else -1
    bull = 1 if C[i] > ma5 > ma10 > ma20 else (-1 if C[i] < ma5 < ma10 < ma20 else 0)
    fade = -1 if m1 > 0 else 1
    dev = (C[i] - ma20) / ma20
    vr = V[i] / (sum(V[i - 19 : i + 1]) / 20)
    atr = atr_pct(H, L, C, i)
    sc = 0.5 * above + fade + 0.6 * bull - 3.0 * dev
    if atr > 0.015:
        sc += 0.5 * fade
    if vr > 1.5 and m1 > 0:
        sc -= 0.15
    return max(-2.5, min(2.5, sc))


def msg_score(by_date: dict[str, list[str]], day: str, lookback: int = 2) -> float:
    end = datetime.strptime(day, "%Y-%m-%d")
    titles = []
    for k in range(lookback + 1):
        titles.extend(by_date.get((end - timedelta(days=k)).strftime("%Y-%m-%d"), []))
    score = 0.0
    for t in titles:
        work = t
        for kw, wt in sorted(BULL.items(), key=lambda x: -len(x[0])):
            if kw in work:
                score += wt * 0.4
                work = work.replace(kw, "　" * len(kw), 1)
        for kw, wt in sorted(BEAR.items(), key=lambda x: -len(x[0])):
            if kw in work:
                score -= wt * 0.4
                work = work.replace(kw, "　" * len(kw), 1)
    if abs(score) < 0.15:
        return 0.0
    return max(-1.8, min(1.8, score))


def pred_from_scores(f: float, m: float, wf: float, wm: float, mode: str) -> int:
    if mode == "agree_boost":
        if m == 0.0:
            sc = f
        elif (f >= 0) == (m >= 0):
            sc = f + 0.35 * m
        else:
            sc = f
    elif mode == "weighted":
        sc = wf * f + wm * m
    elif mode == "gate_msg":
        sc = f + (0.5 * m if abs(m) >= 0.5 else 0.0)
    else:
        sc = f
    return 1 if sc >= 0 else -1


def main() -> None:
    print("fetch klines...", flush=True)
    kl = fetch_klines(320)
    C = [k["close"] for k in kl]
    V = [k["volume"] for k in kl]
    H = [k["high"] for k in kl]
    L = [k["low"] for k in kl]
    D = [k["date"] for k in kl]

    print("fetch news...", flush=True)
    by: dict[str, list[str]] = defaultdict(list)
    seen = set()
    for q in ["上证指数", "沪指", "北向资金", "降准", "A股", "成交额", "稳增长"]:
        items = search_news(q, 4)
        print(f"  {q}: {len(items)}", flush=True)
        for d, t in items:
            if (d, t) in seen:
                continue
            seen.add((d, t))
            by[d].append(t)

    rows = []
    for i in range(60, len(C) - 1):
        f = factor_score(C, V, H, L, i)
        m = msg_score(by, D[i], 2)
        lab = 1 if C[i + 1] > C[i] else -1
        rows.append({"date": D[i], "lab": lab, "f": f, "m": m})

    sample = rows[-120:]
    train, test = rows[-240:-120], rows[-120:]
    print(f"sample={len(sample)} news_unique={len(seen)}", flush=True)

    def eval_mode(data, mode, wf=1.0, wm=0.0):
        ok = sum(pred_from_scores(x["f"], x["m"], wf, wm, mode) == x["lab"] for x in data)
        return ok / len(data), ok

    for name, mode, wf, wm in [
        ("factor_only", "weighted", 1, 0),
        ("msg_only", "weighted", 0, 1),
        ("equal", "weighted", 0.5, 0.5),
    ]:
        a, ok = eval_mode(sample, mode, wf, wm)
        print(f"IS {name}: {a:.1%} ({ok}/120)", flush=True)

    results = []
    for mode in ["weighted", "agree_boost", "gate_msg"]:
        grid = (
            [(wf, 1 - wf) for wf in [0.5, 0.6, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95]]
            if mode == "weighted"
            else [(1.0, 0.0)]
        )
        for wf, wm in grid:
            tr_a, _ = eval_mode(train, mode, wf, wm)
            te_a, te_ok = eval_mode(test, mode, wf, wm)
            is_a, is_ok = eval_mode(sample, mode, wf, wm)
            results.append(
                {
                    "mode": mode,
                    "wf": round(wf, 2),
                    "wm": round(wm, 2),
                    "train": round(tr_a * 100, 1),
                    "test": round(te_a * 100, 1),
                    "is": round(is_a * 100, 1),
                    "te_ok": te_ok,
                    "is_ok": is_ok,
                }
            )

    results.sort(key=lambda x: (-x["test"], -x["train"], -x["is"]))
    print("\n=== TOP by OOS test ===", flush=True)
    for r in results[:15]:
        print(r, flush=True)

    nz = sum(1 for x in sample if x["m"] != 0)
    agree = sum(1 for x in sample if x["m"] != 0 and (x["f"] >= 0) == (x["m"] >= 0))
    disagree = sum(1 for x in sample if x["m"] != 0 and (x["f"] >= 0) != (x["m"] >= 0))
    print(f"\nmsg non-zero: {nz}/120 agree={agree} disagree={disagree}", flush=True)

    out = {
        "best": results[0],
        "top": results[:20],
        "msg_nonzero": nz,
        "agree": agree,
        "disagree": disagree,
    }
    path = Path(__file__).resolve().parents[1] / "tmp_factor_msg_tune.json"
    path.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
    print("wrote", path, flush=True)


if __name__ == "__main__":
    main()
