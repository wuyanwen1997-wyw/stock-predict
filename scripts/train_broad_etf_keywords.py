#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Train keyword weights for 上证指数ETF (510980) message sentiment.

Pulls Eastmoney K-lines + news search, measures next-day edge per keyword,
writes src-tauri/resources/message_weights_broad_etf.json
"""

from __future__ import annotations

import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import defaultdict
from datetime import datetime, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "resources" / "message_weights_broad_etf.json"
SECID = "1.510980"
UA = {
    "User-Agent": (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36"
    ),
    "Referer": "https://so.eastmoney.com/",
    "Accept": "*/*",
}

QUERIES = [
    "A股",
    "上证指数",
    "沪指",
    "北向资金",
    "降准",
    "稳增长",
    "成交额",
    "增量资金",
    "两市成交",
    "沪深300",
    "流动性",
    "风险偏好",
]

CANDIDATES_BULL = [
    "牛市",
    "反弹",
    "回暖",
    "放量",
    "北向流入",
    "北向净买入",
    "降准",
    "降息",
    "宽松",
    "稳增长",
    "政策利好",
    "经济复苏",
    "成交额放大",
    "成交额破万亿",
    "增量资金",
    "翻红",
    "收涨",
    "高开",
    "突破",
    "新高",
    "利好",
    "刺激",
    "活跃",
    "赚钱效应",
    "风险偏好",
    "沪指上涨",
    "上证上涨",
    "收复",
    "反包",
    "护盘",
    "维稳",
    "流动性宽松",
    "偏暖",
    "向好",
    "超预期",
    "政策托底",
    "央行降准",
    "MLF",
    "LPR下调",
    "社融超预期",
    "PMI回升",
    "外资流入",
    "ETF净申购",
    "两融余额升",
    "做多",
]

CANDIDATES_BEAR = [
    "熊市",
    "调整",
    "下跌",
    "承压",
    "北向流出",
    "北向净卖出",
    "收紧",
    "经济疲软",
    "成交低迷",
    "暴跌",
    "翻绿",
    "收跌",
    "低开",
    "跌破",
    "利空",
    "杀跌",
    "亏钱效应",
    "风险偏好下降",
    "沪指下跌",
    "上证下跌",
    "跳水",
    "探底",
    "破位",
    "缩量",
    "加息",
    "地缘",
    "抛售",
    "恐慌",
    "下挫",
    "走弱",
    "回落",
    "失守",
    "外资流出",
    "ETF净赎回",
    "两融余额降",
    "流动性收紧",
    "社融不及预期",
    "PMI回落",
    "政策落空",
]


def get(url: str, retries: int = 4) -> str:
    last: Exception | None = None
    for i in range(retries):
        try:
            req = urllib.request.Request(url, headers=UA)
            with urllib.request.urlopen(req, timeout=20) as r:
                return r.read().decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(0.8 * (i + 1))
    raise RuntimeError(f"GET failed after retries: {url}") from last


def fetch_klines() -> list[dict]:
    # Prefer Tencent — Eastmoney often resets connections from some networks.
    url = (
        "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get"
        "?param=sh510980,day,,,220,qfq"
    )
    kd = json.loads(get(url))
    rows = ((kd.get("data") or {}).get("sh510980") or {}).get("qfqday") or (
        ((kd.get("data") or {}).get("sh510980") or {}).get("day") or []
    )
    out = []
    for row in rows:
        # [date, open, close, high, low, volume]
        out.append({"date": row[0], "close": float(row[2])})
    if out:
        return out

    # Fallback Eastmoney
    url = (
        "https://push2his.eastmoney.com/api/qt/stock/kline/get"
        f"?secid={SECID}&fields1=f1,f2,f3,f4,f5,f6"
        "&fields2=f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61"
        "&klt=101&fqt=1&beg=0&end=20500101&lmt=200"
    )
    kd = json.loads(get(url))
    out = []
    for line in (kd.get("data") or {}).get("klines") or []:
        p = line.split(",")
        out.append({"date": p[0], "close": float(p[2])})
    return out


def search_news(keyword: str, pages: int = 5) -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
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
        except Exception as e:  # noqa: BLE001
            print(f"  search fail {keyword} p{page}: {e}", flush=True)
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
        time.sleep(0.25)
    return out


def titles_as_of(by_date: dict[str, list[str]], day: str, lookback: int = 2) -> list[str]:
    end = datetime.strptime(day, "%Y-%m-%d")
    out: list[str] = []
    for i in range(lookback + 1):
        d = (end - timedelta(days=i)).strftime("%Y-%m-%d")
        out.extend(by_date.get(d, []))
    return out


def eval_kw(
    labels: dict[str, int],
    by_date: dict[str, list[str]],
    kw: str,
    sign_expected: int,
) -> tuple[int, int]:
    hit_correct = 0
    hit_total = 0
    for day, lab in labels.items():
        ts = titles_as_of(by_date, day)
        if not any(kw in t for t in ts):
            continue
        hit_total += 1
        if sign_expected > 0 and lab > 0:
            hit_correct += 1
        if sign_expected < 0 and lab < 0:
            hit_correct += 1
    return hit_total, hit_correct


def weight_from_edge(edge: float) -> float:
    return round(min(1.5, max(0.35, 0.45 + edge * 3.5)), 2)


def main() -> None:
    print("fetching klines...", flush=True)
    klines = fetch_klines()
    labels: dict[str, int] = {}
    for i in range(len(klines) - 1):
        labels[klines[i]["date"]] = (
            1 if klines[i + 1]["close"] > klines[i]["close"] else -1
        )
    up = sum(1 for v in labels.values() if v > 0) / max(1, len(labels))
    print(f"klines={len(klines)} labels={len(labels)} baseline_up={up:.1%}", flush=True)

    all_news: list[tuple[str, str]] = []
    for q in QUERIES:
        items = search_news(q, pages=5)
        print(f"query {q}: {len(items)}", flush=True)
        all_news.extend(items)

    seen: set[tuple[str, str]] = set()
    news: list[tuple[str, str]] = []
    for d, t in all_news:
        k = (d, t)
        if k in seen:
            continue
        seen.add(k)
        news.append((d, t))
    print(f"unique news={len(news)}", flush=True)

    by_date: dict[str, list[str]] = defaultdict(list)
    for d, t in news:
        by_date[d].append(t)

    bull_w: dict[str, float] = {}
    bear_w: dict[str, float] = {}
    report_bull: list[dict] = []
    report_bear: list[dict] = []

    print("\n=== BULL ===", flush=True)
    for kw in CANDIDATES_BULL:
        tot, cor = eval_kw(labels, by_date, kw, 1)
        if tot < 6:
            continue
        acc = cor / tot
        edge = acc - up
        row = {"kw": kw, "n": tot, "acc": round(acc, 3), "edge": round(edge, 3)}
        report_bull.append(row)
        print(f"{kw}\tn={tot}\tacc={acc:.1%}\tedge={edge:+.1%}", flush=True)
        if edge > 0.02:
            bull_w[kw] = weight_from_edge(edge)
        elif edge < -0.05:
            # anti-correlated "bull" word → treat as bearish instead
            bear_w[kw] = weight_from_edge(-edge)

    print("\n=== BEAR ===", flush=True)
    down = 1 - up
    for kw in CANDIDATES_BEAR:
        tot, cor = eval_kw(labels, by_date, kw, -1)
        if tot < 6:
            continue
        acc = cor / tot
        edge = acc - down
        row = {"kw": kw, "n": tot, "acc": round(acc, 3), "edge": round(edge, 3)}
        report_bear.append(row)
        print(f"{kw}\tn={tot}\tacc={acc:.1%}\tedge={edge:+.1%}", flush=True)
        if edge > 0.02:
            bear_w[kw] = weight_from_edge(edge)
        elif edge < -0.05:
            bull_w[kw] = weight_from_edge(-edge)

    # Domain priors for sparse but important policy/flow words
    for kw, w in [
        ("降准", 1.1),
        ("央行降准", 1.2),
        ("北向流入", 0.95),
        ("北向净买入", 1.0),
        ("成交额破万亿", 0.85),
        ("稳增长", 0.7),
        ("LPR下调", 1.0),
        ("社融超预期", 0.9),
        ("PMI回升", 0.85),
        ("政策托底", 0.8),
        ("流动性宽松", 0.85),
        ("风险偏好回升", 0.75),
    ]:
        bull_w.setdefault(kw, w)
    for kw, w in [
        ("北向流出", 1.05),
        ("北向净卖出", 1.1),
        ("暴跌", 1.0),
        ("跳水", 0.95),
        ("破位", 0.9),
        ("杀跌", 0.95),
        ("亏钱效应", 0.85),
        ("外资流出", 0.9),
        ("流动性收紧", 0.85),
        ("风险偏好下降", 0.85),
    ]:
        bear_w.setdefault(kw, w)

    # Resolve conflicts: prefer empirical side
    for kw in list(bull_w.keys()):
        if kw in bear_w:
            if bear_w[kw] >= bull_w[kw]:
                del bull_w[kw]
            else:
                del bear_w[kw]

    # Drop ultra-generic noise if empirically weak
    for noise in ("利好", "利空", "上涨", "调整", "回暖"):
        bull_w.pop(noise, None)
        bear_w.pop(noise, None)

    payload = {
        "profile": "broad_etf_sse",
        "target": "510980",
        "lookback_days": 2,
        "baseline_up": round(up, 4),
        "bullish": dict(sorted(bull_w.items(), key=lambda x: -x[1])),
        "bearish": dict(sorted(bear_w.items(), key=lambda x: -x[1])),
        "extra_queries": [
            "上证指数",
            "沪指",
            "A股 市场",
            "北向资金",
            "两市成交",
            "降准 稳增长",
            "成交额破万亿",
            "风险偏好",
        ],
        "hit_weight": 0.4,
        "scale": 0.95,
        "note": "empirical edge on 510980 next-day + domain priors; re-run scripts/train_broad_etf_keywords.py",
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    report_path = OUT.with_name("message_weights_broad_etf_report.json")
    report_path.write_text(
        json.dumps({"bull": report_bull, "bear": report_bear}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"\nwrote {OUT}", flush=True)
    print(f"report {report_path}", flush=True)
    print(f"bull={len(bull_w)} bear={len(bear_w)}", flush=True)


if __name__ == "__main__":
    try:
        main()
    except urllib.error.URLError as e:
        raise SystemExit(f"network error: {e}") from e
