import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { cn, formatPct } from "@/lib/utils";
import {
  floatingPnl,
  GROUP_HOLDINGS_MIRROR,
  newJournalId,
  todayIsoDate,
} from "@/lib/pool";
import { useStockStore } from "@/stores/stockStore";
import { useMonitorStore } from "@/stores/monitorStore";

export function ReviewPage() {
  const navigate = useNavigate();
  const poolItems = useStockStore((s) => s.poolItems);
  const holdings = useStockStore((s) => s.holdings);
  const journalEntries = useStockStore((s) => s.journalEntries);
  const stocks = useStockStore((s) => s.stocks);
  const hotStocks = useStockStore((s) => s.hotStocks);
  const backtest = useStockStore((s) => s.backtest);
  const bsMarkers = useStockStore((s) => s.bsMarkers);
  const selectedStock = useStockStore((s) => s.selectedStock);
  const prediction = useStockStore((s) => s.prediction);
  const addJournalEntry = useStockStore((s) => s.addJournalEntry);
  const removeJournalEntry = useStockStore((s) => s.removeJournalEntry);
  const alerts = useMonitorStore((s) => s.alerts);

  const [note, setNote] = useState("");
  const [noteCode, setNoteCode] = useState("");

  const quoteMap = useMemo(() => {
    const m = new Map([...hotStocks, ...stocks].map((s) => [s.code, s]));
    return m;
  }, [stocks, hotStocks]);

  const poolCodes = useMemo(() => {
    const set = new Set(
      poolItems
        .filter((i) => i.groupId !== GROUP_HOLDINGS_MIRROR)
        .map((i) => i.code),
    );
    return [...set];
  }, [poolItems]);

  const poolDist = useMemo(() => {
    let up = 0;
    let down = 0;
    let flat = 0;
    for (const code of poolCodes) {
      const pct = quoteMap.get(code)?.change_pct;
      if (pct == null) continue;
      if (pct > 0) up += 1;
      else if (pct < 0) down += 1;
      else flat += 1;
    }
    return { up, down, flat, n: poolCodes.length };
  }, [poolCodes, quoteMap]);

  const holdingsPnl = holdings.reduce((acc, h) => {
    const p = floatingPnl(h.cost, h.qty, quoteMap.get(h.code)?.price);
    return acc + (p?.pnl ?? 0);
  }, 0);

  const predictCheck = useMemo(() => {
    if (!prediction || !selectedStock) return null;
    const live = quoteMap.get(selectedStock.code);
    if (live?.change_pct == null) {
      return {
        code: selectedStock.code,
        name: selectedStock.name,
        predicted: prediction.predicted,
        actual: null as string | null,
        ok: null as boolean | null,
      };
    }
    const actual = live.change_pct > 0 ? "up" : live.change_pct < 0 ? "down" : "flat";
    const pred = (prediction.predicted || "").toLowerCase();
    const ok =
      pred === "up" && actual === "up"
        ? true
        : pred === "down" && actual === "down"
          ? true
          : pred === "flat"
            ? actual === "flat"
            : null;
    return {
      code: selectedStock.code,
      name: selectedStock.name,
      predicted: prediction.predicted,
      actual: `${formatPct(live.change_pct)}`,
      ok,
    };
  }, [prediction, selectedStock, quoteMap]);

  const recentBs = [...bsMarkers].slice(-5).reverse();
  const todayNotes = journalEntries
    .filter((j) => j.date === todayIsoDate())
    .sort((a, b) => b.id.localeCompare(a.id));

  return (
    <div className="h-full min-h-0 overflow-y-auto px-3 py-3">
      <h1 className="text-base font-semibold">复盘</h1>
      <p className="mt-1 text-[11px] text-slate-500">盘后摘要 · 预测对照 · 笔记（本地）</p>

      <section className="mt-4 grid grid-cols-3 gap-2">
        <SummaryCard
          label="池涨跌"
          value={`${poolDist.up}↑ ${poolDist.down}↓`}
          sub={`${poolDist.n} 只`}
        />
        <SummaryCard
          label="仓浮盈"
          value={holdings.length ? holdingsPnl.toFixed(0) : "—"}
          sub={`${holdings.length} 笔`}
          tone={holdings.length ? (holdingsPnl >= 0 ? "up" : "down") : undefined}
        />
        <SummaryCard label="预警" value={String(alerts.length)} sub="今日会话" />
      </section>

      <section className="mt-4 rounded-xl border border-white/5 bg-slate-900/40 p-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm text-slate-200">预测对照</h2>
          <Link to="/pool" className="text-[11px] text-cyan-400">
            调池
          </Link>
        </div>
        {!predictCheck ? (
          <p className="mt-2 text-xs text-slate-500">
            打开个股诊股后，可在此对照当日涨跌（轻量会话态，非历史快照库）。
          </p>
        ) : (
          <div className="mt-2 flex items-center gap-2 text-sm">
            <button
              type="button"
              className="text-left text-slate-200"
              onClick={() => navigate(`/stock/${predictCheck.code}`)}
            >
              {predictCheck.name}
            </button>
            <span className="text-slate-500">预测 {predictCheck.predicted}</span>
            <span className="text-slate-500">实际 {predictCheck.actual ?? "待行情"}</span>
            <span
              className={cn(
                "ml-auto text-xs",
                predictCheck.ok === true && "text-emerald-400",
                predictCheck.ok === false && "text-rose-400",
                predictCheck.ok == null && "text-slate-500",
              )}
            >
              {predictCheck.ok === true ? "✓" : predictCheck.ok === false ? "✗" : "—"}
            </span>
          </div>
        )}
        {backtest && (
          <p className="mt-2 text-[11px] text-slate-500">
            最近回测方向准确率 {(backtest.direction_accuracy * 100).toFixed(1)}% ·{" "}
            <button
              type="button"
              className="text-cyan-400"
              onClick={() =>
                selectedStock && navigate(`/stock/${selectedStock.code}?tab=diagnose`)
              }
            >
              查看诊股回测
            </button>
          </p>
        )}
      </section>

      <section className="mt-4 rounded-xl border border-white/5 bg-slate-900/40 p-3">
        <h2 className="text-sm text-slate-200">买卖点回顾</h2>
        <p className="mt-1 text-[11px] text-slate-500">
          来自当前个股日 K MACD B/S（研究口径）
        </p>
        <ul className="mt-2 space-y-1 text-xs text-slate-300">
          {recentBs.length === 0 && <li className="text-slate-500">暂无 · 先打开个股行情</li>}
          {recentBs.map((m) => (
            <li key={`${m.date}-${m.kind}`}>
              <span className={m.kind === "buy" ? "text-rose-400" : "text-emerald-400"}>
                {m.kind === "buy" ? "B" : "S"}
              </span>{" "}
              {m.date}
            </li>
          ))}
        </ul>
      </section>

      <section className="mt-4 rounded-xl border border-white/5 bg-slate-900/40 p-3">
        <h2 className="text-sm text-slate-200">笔记</h2>
        <textarea
          className="mt-2 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
          rows={3}
          placeholder="今日想法…"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
        <div className="mt-2 flex gap-2">
          <input
            className="min-w-0 flex-1 rounded-lg border border-white/10 bg-slate-950 px-2 py-1 text-xs"
            placeholder="关联代码（可选）"
            value={noteCode}
            onChange={(e) => setNoteCode(e.target.value)}
          />
          <button
            type="button"
            className="rounded-lg bg-emerald-500/20 px-3 py-1 text-xs text-emerald-300"
            onClick={() => {
              const body = note.trim();
              if (!body) return;
              addJournalEntry({
                id: newJournalId(),
                date: todayIsoDate(),
                code: noteCode.trim() || null,
                body,
                extraJson: "{}",
              });
              setNote("");
              setNoteCode("");
            }}
          >
            保存
          </button>
        </div>
        <ul className="mt-3 space-y-2">
          {todayNotes.map((j) => (
            <li
              key={j.id}
              className="rounded-lg bg-slate-950/60 px-2 py-2 text-xs text-slate-300"
            >
              <div className="flex justify-between gap-2">
                <span className="text-slate-500">
                  {j.date}
                  {j.code ? ` · ${j.code}` : ""}
                </span>
                <button
                  type="button"
                  className="text-rose-400"
                  onClick={() => removeJournalEntry(j.id)}
                >
                  删
                </button>
              </div>
              <p className="mt-1 whitespace-pre-wrap">{j.body}</p>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

function SummaryCard({
  label,
  value,
  sub,
  tone,
}: {
  label: string;
  value: string;
  sub: string;
  tone?: "up" | "down";
}) {
  return (
    <div className="rounded-xl border border-white/5 bg-slate-900/50 px-2 py-2">
      <div className="text-[10px] text-slate-500">{label}</div>
      <div
        className={cn(
          "mt-0.5 font-mono text-sm font-semibold tabular-nums",
          tone === "up" && "text-rose-400",
          tone === "down" && "text-emerald-400",
          !tone && "text-slate-100",
        )}
      >
        {value}
      </div>
      <div className="text-[10px] text-slate-600">{sub}</div>
    </div>
  );
}
