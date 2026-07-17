import { motion } from "framer-motion";
import { cn, formatPct } from "@/lib/utils";
import type { BacktestResult } from "@/types";

interface Props {
  result: BacktestResult | null;
  loading?: boolean;
}

const directionLabel: Record<string, string> = {
  up: "涨",
  down: "跌",
};

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
      {hint && <div className="mt-1 text-[11px] text-slate-500">{hint}</div>}
    </div>
  );
}

export function BacktestPanel({ result, loading }: Props) {
  const recent = result?.records.slice(-12).reverse() ?? [];
  const hcRecent = result?.records.filter((r) => r.high_confidence).slice(-8).reverse() ?? [];

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
          Walk-forward · 回看 {result?.lookback_days ?? 50} 日 · 高置信阈值 ≥
          {result?.high_confidence_threshold?.toFixed(0) ?? 60}%
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
      ) : !result || result.total_samples === 0 ? (
        <div className="flex h-40 items-center justify-center text-sm text-slate-500">
          {result?.summary ?? "暂无回测数据"}
        </div>
      ) : (
        <>
          <div className="mb-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <MetricCard
              label={result.selective_mode ? "有效信号准确率" : "整体准确率"}
              value={`${result.direction_accuracy.toFixed(1)}%`}
              hint={
                result.selective_mode
                  ? `${result.total_samples} 次有效 · 全样本 ${(result.all_day_accuracy ?? 0).toFixed(1)}%`
                  : `${result.total_samples} 次样本`
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
                result.high_confidence_samples > 0
                  ? `${result.high_confidence_samples} 次高置信`
                  : "暂无高置信样本"
              }
              accent="cyan"
            />
            <MetricCard
              label="看涨命中率"
              value={`${result.up_hit_rate.toFixed(1)}%`}
              accent="amber"
            />
            <MetricCard
              label="看跌命中率"
              value={`${result.down_hit_rate.toFixed(1)}%`}
              accent="violet"
            />
          </div>

          <div className="mb-4 rounded-xl border border-white/5 bg-slate-800/30 px-4 py-3 text-sm leading-relaxed text-slate-300">
            {result.summary}
          </div>

          {hcRecent.length > 0 && (
            <div className="mb-4">
              <div className="mb-2 text-xs font-medium text-cyan-300/80">最近高置信信号</div>
              <div className="overflow-x-auto rounded-xl border border-cyan-500/15">
                <table className="min-w-full text-left text-xs">
                  <thead className="bg-cyan-500/5 text-slate-500">
                    <tr>
                      <th className="px-3 py-2 font-medium">预测日</th>
                      <th className="px-3 py-2 font-medium">方向</th>
                      <th className="px-3 py-2 font-medium">实际</th>
                      <th className="px-3 py-2 font-medium">领先概率</th>
                      <th className="px-3 py-2 font-medium">结果</th>
                    </tr>
                  </thead>
                  <tbody>
                    {hcRecent.map((row) => {
                      const lead =
                        row.predicted === "up" ? row.up_probability : row.down_probability;
                      return (
                        <tr
                          key={`hc-${row.predict_date}`}
                          className="border-t border-white/5 text-slate-300"
                        >
                          <td className="px-3 py-2 font-mono text-slate-400">
                            {row.predict_date.slice(5)}
                          </td>
                          <td className="px-3 py-2">
                            {directionLabel[row.predicted]}
                          </td>
                          <td className="px-3 py-2">
                            {directionLabel[row.actual]}
                          </td>
                          <td className="px-3 py-2 font-mono tabular-nums text-cyan-300/80">
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
            </div>
          )}

          <div className="overflow-x-auto rounded-xl border border-white/5">
            <table className="min-w-full text-left text-xs">
              <thead className="bg-slate-800/50 text-slate-500">
                <tr>
                  <th className="px-3 py-2 font-medium">预测日</th>
                  <th className="px-3 py-2 font-medium">预测</th>
                  <th className="px-3 py-2 font-medium">实际</th>
                  <th className="px-3 py-2 font-medium">涨跌</th>
                  <th className="px-3 py-2 font-medium">概率</th>
                  <th className="px-3 py-2 font-medium">结果</th>
                </tr>
              </thead>
              <tbody>
                {recent.map((row) => {
                  const upActual = row.actual === "up";
                  const downActual = row.actual === "down";

                  return (
                    <tr
                      key={row.predict_date}
                      className={cn(
                        "border-t border-white/5 text-slate-300",
                        row.high_confidence && "bg-cyan-500/5",
                      )}
                    >
                      <td className="px-3 py-2 font-mono text-slate-400">
                        {row.predict_date.slice(5)}
                        {row.high_confidence && (
                          <span className="ml-1 text-[10px] text-cyan-400">高</span>
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
        </>
      )}
    </motion.div>
  );
}
