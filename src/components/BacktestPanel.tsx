import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { cn, formatPct } from "@/lib/utils";
import type { BacktestRecord, BacktestResult } from "@/types";

interface Props {
  result: BacktestResult | null;
  loading?: boolean;
}

const PAGE_SIZE = 20;
/** 与后端 backtest.rs ACTIONABLE_LEAD 对齐 */
const ACTIONABLE_LEAD = 55;

const directionLabel: Record<string, string> = {
  up: "涨",
  down: "跌",
};

function leadProb(row: BacktestRecord): number {
  return row.predicted === "up" ? row.up_probability : row.down_probability;
}

function isActionable(row: BacktestRecord): boolean {
  return leadProb(row) >= ACTIONABLE_LEAD;
}

function signalKind(row: BacktestRecord): "hc" | "act" | "weak" {
  if (row.high_confidence) return "hc";
  if (isActionable(row)) return "act";
  return "weak";
}

function MetricCard({
  label,
  value,
  hint,
  accent,
}: {
  label: string;
  value: string;
  hint?: string;
  accent?: "emerald" | "cyan" | "amber" | "violet";
}) {
  const accentClass = {
    emerald: "border-emerald-500/20 bg-emerald-500/5 text-emerald-300",
    cyan: "border-cyan-500/20 bg-cyan-500/5 text-cyan-300",
    amber: "border-amber-500/20 bg-amber-500/5 text-amber-300",
    violet: "border-violet-500/20 bg-violet-500/5 text-violet-300",
  }[accent ?? "emerald"];

  return (
    <div className={cn("rounded-xl border px-4 py-3", accentClass)}>
      <div className="text-xs text-slate-500">{label}</div>
      <div className="mt-1 font-mono text-2xl font-bold tabular-nums">{value}</div>
      {hint && <div className="mt-1 text-[11px] leading-snug text-slate-500">{hint}</div>}
    </div>
  );
}

export function BacktestPanel({ result, loading }: Props) {
  const [page, setPage] = useState(1);

  const allRecords = useMemo(() => {
    if (!result?.records?.length) return [];
    // 新→旧，完整每日回归
    return [...result.records].reverse();
  }, [result]);

  const totalPages = Math.max(1, Math.ceil(allRecords.length / PAGE_SIZE));

  useEffect(() => {
    setPage(1);
  }, [result?.stock?.code, result?.records?.length, result?.summary]);

  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

  const pageRecords = useMemo(() => {
    const start = (page - 1) * PAGE_SIZE;
    return allRecords.slice(start, start + PAGE_SIZE);
  }, [allRecords, page]);

  const stats = useMemo(() => {
    if (!result) return null;
    const all = result.records.length;
    const act = result.records.filter(isActionable).length;
    const hc = result.records.filter((r) => r.high_confidence).length;
    const actCorrect = result.records.filter((r) => isActionable(r) && r.correct).length;
    const hcCorrect = result.records.filter((r) => r.high_confidence && r.correct).length;
    const allCorrect = result.records.filter((r) => r.correct).length;
    const upAll = result.records.filter((r) => r.predicted === "up");
    const downAll = result.records.filter((r) => r.predicted === "down");
    const upAct = upAll.filter(isActionable);
    const downAct = downAll.filter(isActionable);
    return {
      all,
      act,
      hc,
      actCorrect,
      hcCorrect,
      allCorrect,
      upOk: upAll.filter((r) => r.correct).length,
      upN: upAll.length,
      downOk: downAll.filter((r) => r.correct).length,
      downN: downAll.length,
      upActOk: upAct.filter((r) => r.correct).length,
      upActN: upAct.length,
      downActOk: downAct.filter((r) => r.correct).length,
      downActN: downAct.length,
    };
  }, [result]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.1 }}
      className="rounded-2xl border border-white/5 bg-slate-900/50 p-5 backdrop-blur-sm"
    >
      <div className="mb-4">
        <h3 className="text-base font-semibold text-slate-100">历史预测回测</h3>
        <p className="mt-1 text-xs text-slate-500">
          Walk-forward · 每个交易日预测次日涨跌 1 次 · 特征回看{" "}
          {result?.lookback_days ?? 50} 日（≠回测天数）· 高置信 ≥
          {result?.high_confidence_threshold?.toFixed(0) ?? 60}% · 有效 ≥{ACTIONABLE_LEAD}%
        </p>
      </div>

      {loading ? (
        <div className="space-y-3">
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="animate-shimmer h-20 rounded-xl" />
            ))}
          </div>
          <div className="animate-shimmer h-48 rounded-xl" />
        </div>
      ) : !result || result.records.length === 0 ? (
        <div className="flex h-40 items-center justify-center text-sm text-slate-500">
          {result?.summary ?? "暂无回测数据"}
        </div>
      ) : (
        <>
          <div className="mb-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <MetricCard
              label="整体准确率"
              value={`${result.direction_accuracy.toFixed(1)}%`}
              hint={
                stats
                  ? `全样本命中 ${stats.allCorrect}/${stats.all} 次 · 有效 ${stats.actCorrect}/${stats.act}（${result.actionable_accuracy.toFixed(1)}%）`
                  : undefined
              }
              accent="emerald"
            />
            <MetricCard
              label="高置信准确率"
              value={
                result.high_confidence_samples > 0
                  ? `${result.high_confidence_accuracy.toFixed(1)}%`
                  : "—"
              }
              hint={
                stats && stats.hc > 0
                  ? `命中 ${stats.hcCorrect}/${stats.hc} 次高置信`
                  : "暂无高置信样本"
              }
              accent="cyan"
            />
            <MetricCard
              label="看涨命中率"
              value={`${result.up_hit_rate.toFixed(1)}%`}
              hint={
                stats
                  ? `全量 ${stats.upOk}/${stats.upN} · 有效 ${stats.upActOk}/${stats.upActN}（${(result.up_hit_rate_actionable ?? 0).toFixed(1)}%）`
                  : undefined
              }
              accent="amber"
            />
            <MetricCard
              label="看跌命中率"
              value={`${result.down_hit_rate.toFixed(1)}%`}
              hint={
                stats
                  ? `全量 ${stats.downOk}/${stats.downN} · 有效 ${stats.downActOk}/${stats.downActN}（${(result.down_hit_rate_actionable ?? 0).toFixed(1)}%）`
                  : undefined
              }
              accent="violet"
            />
          </div>

          <div className="mb-4 space-y-2 rounded-xl border border-white/5 bg-slate-800/30 px-4 py-3 text-sm leading-relaxed text-slate-300">
            <p>{result.summary}</p>
            <p className="text-xs text-slate-500">
              主指标按全部预测日统计。「回看 N 日」是特征窗口，不是回测天数。有效 =
              领先概率 ≥{ACTIONABLE_LEAD}%；高置信 = 领先概率 ≥
              {result.high_confidence_threshold.toFixed(0)}
              %。看涨/看跌卡片同时给出全量与有效命中。
            </p>
          </div>

          <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
            <div className="text-xs font-medium text-slate-400">
              每日回归预测
              <span className="ml-2 font-normal text-slate-500">
                共 {allRecords.length} 条 · 第 {page}/{totalPages} 页 · 每页 {PAGE_SIZE} 条
              </span>
            </div>
            <div className="flex items-center gap-1.5 text-[10px] text-slate-500">
              <span className="rounded bg-cyan-500/15 px-1.5 py-0.5 text-cyan-300">高置信</span>
              <span className="rounded bg-emerald-500/15 px-1.5 py-0.5 text-emerald-300">
                有效
              </span>
              <span className="rounded bg-slate-700/80 px-1.5 py-0.5 text-slate-400">弱信号</span>
            </div>
          </div>

          <div className="overflow-x-auto rounded-xl border border-white/5">
            <table className="min-w-full text-left text-xs">
              <thead className="bg-slate-800/50 text-slate-500">
                <tr>
                  <th className="px-3 py-2 font-medium">#</th>
                  <th className="px-3 py-2 font-medium">基准日</th>
                  <th className="px-3 py-2 font-medium">预测日</th>
                  <th className="px-3 py-2 font-medium">信号</th>
                  <th className="px-3 py-2 font-medium">预测</th>
                  <th className="px-3 py-2 font-medium">实际</th>
                  <th className="px-3 py-2 font-medium">涨跌</th>
                  <th className="px-3 py-2 font-medium">涨/跌概率</th>
                  <th className="px-3 py-2 font-medium">领先</th>
                  <th className="px-3 py-2 font-medium">结果</th>
                </tr>
              </thead>
              <tbody>
                {pageRecords.map((row, idx) => {
                  const upActual = row.actual === "up";
                  const downActual = row.actual === "down";
                  const lead = leadProb(row);
                  const kind = signalKind(row);
                  const globalIndex = (page - 1) * PAGE_SIZE + idx + 1;

                  return (
                    <tr
                      key={`${row.date}-${row.predict_date}`}
                      className={cn(
                        "border-t border-white/5 text-slate-300",
                        kind === "hc" && "bg-cyan-500/5",
                        kind === "act" && "bg-emerald-500/[0.03]",
                      )}
                    >
                      <td className="px-3 py-2 font-mono text-slate-600">{globalIndex}</td>
                      <td className="px-3 py-2 font-mono text-slate-500">{row.date}</td>
                      <td className="px-3 py-2 font-mono text-slate-400">
                        {row.predict_date}
                      </td>
                      <td className="px-3 py-2">
                        {kind === "hc" && (
                          <span className="rounded bg-cyan-500/15 px-1.5 py-0.5 text-[10px] text-cyan-300">
                            高置信
                          </span>
                        )}
                        {kind === "act" && (
                          <span className="rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] text-emerald-300">
                            有效
                          </span>
                        )}
                        {kind === "weak" && (
                          <span className="rounded bg-slate-700/80 px-1.5 py-0.5 text-[10px] text-slate-400">
                            弱信号
                          </span>
                        )}
                      </td>
                      <td className="px-3 py-2">
                        {directionLabel[row.predicted] ?? row.predicted}
                      </td>
                      <td
                        className={cn(
                          "px-3 py-2",
                          upActual && "text-rose-400",
                          downActual && "text-emerald-400",
                        )}
                      >
                        {directionLabel[row.actual] ?? row.actual}
                      </td>
                      <td
                        className={cn(
                          "px-3 py-2 font-mono tabular-nums",
                          row.change_pct > 0 && "text-rose-400",
                          row.change_pct < 0 && "text-emerald-400",
                        )}
                      >
                        {formatPct(row.change_pct)}
                      </td>
                      <td className="px-3 py-2 font-mono tabular-nums text-slate-500">
                        {row.up_probability.toFixed(0)} / {row.down_probability.toFixed(0)}
                      </td>
                      <td className="px-3 py-2 font-mono tabular-nums text-slate-400">
                        {lead.toFixed(0)}%
                      </td>
                      <td className="px-3 py-2">
                        {row.correct ? (
                          <span className="text-emerald-400">✓</span>
                        ) : (
                          <span className="text-slate-600">✗</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {totalPages > 1 && (
            <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
              <button
                type="button"
                disabled={page <= 1}
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                className={cn(
                  "rounded-lg border px-3 py-1.5 text-xs transition",
                  page <= 1
                    ? "cursor-not-allowed border-white/5 text-slate-600"
                    : "border-white/10 text-slate-300 hover:border-white/20 hover:bg-white/5",
                )}
              >
                上一页
              </button>
              <div className="flex flex-wrap items-center justify-center gap-1">
                {Array.from({ length: totalPages }, (_, i) => i + 1).map((p) => (
                  <button
                    key={p}
                    type="button"
                    onClick={() => setPage(p)}
                    className={cn(
                      "min-w-8 rounded-md px-2 py-1 font-mono text-xs tabular-nums transition",
                      p === page
                        ? "bg-cyan-500/20 text-cyan-200"
                        : "text-slate-500 hover:bg-white/5 hover:text-slate-300",
                    )}
                  >
                    {p}
                  </button>
                ))}
              </div>
              <button
                type="button"
                disabled={page >= totalPages}
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                className={cn(
                  "rounded-lg border px-3 py-1.5 text-xs transition",
                  page >= totalPages
                    ? "cursor-not-allowed border-white/5 text-slate-600"
                    : "border-white/10 text-slate-300 hover:border-white/20 hover:bg-white/5",
                )}
              >
                下一页
              </button>
            </div>
          )}
        </>
      )}
    </motion.div>
  );
}
