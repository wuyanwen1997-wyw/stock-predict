#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Offline search: find message-only rules that maximize next-day accuracy on 510980."""

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
    "User-Agent": (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36"
    ),
    "Referer": "https://so.eastmoney.com/",
}

BULL = {
    "央行降准": 1.2,
    "降准": 1.1,
    "LPR下调": 1.0,
    "北向净买入": 1.0,
    "北向流入": 0.95,
    "社融超预期": 0.9,
    "成交额破万亿": 0.85,
    "PMI回升": 0.85,
    "政策托底": 0.8,
    "稳增长": 0.7,
    "流动性宽松": 0.85,
    "赚钱效应": 0.7,
    "风险偏好回升": 0.75,
}
# fade / true bear
BEAR = {
    "北向净卖出": 1.1,
    "北向流出": 1.05,
    "暴跌": 1.0,
    "跳水": 0.95,
    "杀跌": 0.95,
    "破位": 0.9,
    "回落": 1.2,
    "承压": 1.05,
    "缩量": 1.05,
    "走弱": 1.05,
    "下跌": 0.95,
    "跌破": 0.7,
    "亏钱效应": 0.85,
    "流动性收紧": 0.85,
    "风险偏好下降": 0.85,
    "反弹": 1.15,
    "放量": 0.9,
    "收涨": 0.95,
    "新高": 0.95,
    "活跃": 0.95,
    "增量资金": 0.75,
}

QUERIES = [
    "上证指数",
    "沪指",
    "A股",
    "北向资金",
    "降准",
    "稳增长",
    "成交额",
    "两市成交",
    "风险偏好",
]


def get(url: str, retries: int = 4) -> str:
    last = None
    for i in range(retries):
        try:
            req = urllib.request.Request(url, headers=UA)
            with urllib.request.urlopen(req, timeout=20) as r:
                return r.read().decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(0.6 * (i + 1))
    raise RuntimeError(str(last))


def fetch_klines() -> list[dict]:
    url = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sh510980,day,,,160,qfq"
    kd = json.loads(get(url))
    rows = ((kd.get("data") or {}).get("sh510980") or {}).get("qfqday") or (
        ((kd.get("data") or {}).get("sh510980") or {}).get("day") or []
    )
    return [{"date": r[0], "close": float(r[2])} for r in rows]


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
            d = json.loads(re.sub(r"[\x00-\x1f]", " ", body))
        arr = ((d.get("result") or {}).get("cmsArticleWebOld")) or []
        if not arr:
            break
        for it in arr:
            title = re.sub(r"<[^>]+>", "", it.get("title") or "")
            date = (it.get("date") or "")[:10]
            if title and date:
                out.append((date, title))
        time.sleep(0.2)
    return out


def titles_window(by_date: dict[str, list[str]], day: str, lookback: int) -> list[str]:
    end = datetime.strptime(day, "%Y-%m-%d")
    out = []
    for i in range(lookback + 1):
        d = (end - timedelta(days=i)).strftime("%Y-%m-%d")
        out.extend(by_date.get(d, []))
    return out


def score_titles(titles: list[str], bull: dict, bear: dict) -> float:
    score = 0.0
    for t in titles:
        work = t
        b = 0.0
        e = 0.0
        for kw, wt in sorted(bull.items(), key=lambda x: -len(x[0])):
            if kw in work:
                b += wt
                work = work.replace(kw, "　" * len(kw), 1)
        for kw, wt in sorted(bear.items(), key=lambda x: -len(x[0])):
            if kw in work:
                e += wt
                work = work.replace(kw, "　" * len(kw), 1)
        if b == 0 and e == 0:
            continue
        score += (b - e) * 0.4
    return score


def eval_preds(labels: dict[str, int], preds: dict[str, int]) -> tuple[float, int, int]:
    ok = tot = 0
    for d, lab in labels.items():
        if d not in preds:
            continue
        tot += 1
        if preds[d] == lab:
            ok += 1
    return (ok / tot if tot else 0.0), ok, tot


def main() -> None:
    print("fetch klines...", flush=True)
    klines = fetch_klines()
    labels = {}
    closes = {}
    for i in range(len(klines) - 1):
        d = klines[i]["date"]
        labels[d] = 1 if klines[i + 1]["close"] > klines[i]["close"] else -1
        closes[d] = klines[i]["close"]
    # yesterday return for mean-reversion prior
    yret = {}
    for i in range(1, len(klines) - 1):
        d = klines[i]["date"]
        yret[d] = 1 if klines[i]["close"] > klines[i - 1]["close"] else -1

    print(f"labels={len(labels)}", flush=True)
    all_news = []
    for q in QUERIES:
        items = search_news(q, 4)
        print(f"  {q}: {len(items)}", flush=True)
        all_news.extend(items)
    seen = set()
    by_date = defaultdict(list)
    for d, t in all_news:
        if (d, t) in seen:
            continue
        seen.add((d, t))
        by_date[d].append(t)
    print(f"unique={len(seen)}", flush=True)

    # Use last 79 trading days like UI sample
    days = sorted(labels.keys())[-79:]

    results = []

    # Strategy variants
    for lookback in (1, 2, 3, 4, 5, 7):
        for thr in (0.0, 0.15, 0.25, 0.35, 0.5, 0.8):
            for fallback in ("up", "down", "fade_yest", "follow_yest", "skip"):
                preds = {}
                for d in days:
                    titles = titles_window(by_date, d, lookback)
                    s = score_titles(titles, BULL, BEAR)
                    if abs(s) >= thr:
                        preds[d] = 1 if s > 0 else -1
                    else:
                        if fallback == "up":
                            preds[d] = 1
                        elif fallback == "down":
                            preds[d] = -1
                        elif fallback == "fade_yest":
                            preds[d] = -yret.get(d, 1)
                        elif fallback == "follow_yest":
                            preds[d] = yret.get(d, 1)
                        else:
                            continue  # skip day
                acc, ok, tot = eval_preds({d: labels[d] for d in days}, preds)
                if tot < 20:
                    continue
                results.append(
                    {
                        "lookback": lookback,
                        "thr": thr,
                        "fallback": fallback,
                        "acc": round(acc * 100, 1),
                        "ok": ok,
                        "tot": tot,
                    }
                )

    # Bear-only: any bear hit → down, else up / fade
    for lookback in (1, 2, 3, 5):
        for fallback in ("up", "fade_yest", "follow_yest"):
            preds = {}
            for d in days:
                titles = titles_window(by_date, d, lookback)
                text = " ".join(titles)
                bear_hit = any(k in text for k in BEAR)
                bull_hit = any(k in text for k in BULL)
                if bear_hit and not bull_hit:
                    preds[d] = -1
                elif bull_hit and not bear_hit:
                    preds[d] = 1
                elif bear_hit and bull_hit:
                    # net score
                    s = score_titles(titles, BULL, BEAR)
                    preds[d] = 1 if s >= 0 else -1
                else:
                    if fallback == "up":
                        preds[d] = 1
                    elif fallback == "fade_yest":
                        preds[d] = -yret.get(d, 1)
                    else:
                        preds[d] = yret.get(d, 1)
            acc, ok, tot = eval_preds({d: labels[d] for d in days}, preds)
            results.append(
                {
                    "lookback": lookback,
                    "thr": "bear_rule",
                    "fallback": fallback,
                    "acc": round(acc * 100, 1),
                    "ok": ok,
                    "tot": tot,
                }
            )

    # Pure fade yesterday (baseline without news)
    preds = {d: -yret.get(d, 1) for d in days}
    acc, ok, tot = eval_preds({d: labels[d] for d in days}, preds)
    results.append(
        {"lookback": 0, "thr": "none", "fallback": "fade_yest_only", "acc": round(acc * 100, 1), "ok": ok, "tot": tot}
    )
    preds = {d: 1 for d in days}
    acc, ok, tot = eval_preds({d: labels[d] for d in days}, preds)
    results.append(
        {"lookback": 0, "thr": "none", "fallback": "always_up", "acc": round(acc * 100, 1), "ok": ok, "tot": tot}
    )

    results.sort(key=lambda x: (-x["acc"], -x["tot"]))
    print("\n=== TOP 25 ===", flush=True)
    for r in results[:25]:
        print(r, flush=True)
    print("\n=== >=60% ===", flush=True)
    ge = [r for r in results if r["acc"] >= 60 and r["tot"] >= 40]
    for r in ge[:20]:
        print(r, flush=True)
    if not ge:
        print("NONE with tot>=40", flush=True)
        ge2 = [r for r in results if r["acc"] >= 60]
        for r in ge2[:15]:
            print("loose", r, flush=True)

    out = Path(__file__).resolve().parents[1] / "tmp_msg_opt.json"
    out.write_text(json.dumps({"top": results[:40], "ge60": ge[:20]}, ensure_ascii=False, indent=2), encoding="utf-8")
    print("wrote", out, flush=True)


if __name__ == "__main__":
    main()
