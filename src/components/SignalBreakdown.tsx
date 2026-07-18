import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import type { SignalContribution } from "@/types";

interface Props {
  signals: SignalContribution[];
}

export function SignalBreakdown({ signals }: Props) {
  if (!signals.length) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      className="rounded-2xl border border-white/5 bg-slate-900/50 p-5 backdrop-blur-sm"
    >
      <div className="mb-4">
        <h3 className="text-base font-semibold text-slate-100">信号明细</h3>
        <p className="mt-1 text-xs text-slate-500">
          各信号源独立打分后按权重融合。宽基上消息面仅在「有方向且与多因子一致」时计入；
          若明细显示「未计入」，开关消息面主概率可以不变。
        </p>
      </div>

      <div className="space-y-2">
        {signals.map((s) => {
          const bullish = s.up_probability >= s.down_probability;
          return (
            <div
              key={s.id}
              className="rounded-xl border border-white/5 bg-slate-800/30 px-3 py-2.5"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-slate-200">{s.name}</span>
                  <span className="text-[10px] text-slate-500">{s.category}</span>
                  <span
                    className={cn(
                      "rounded px-1.5 py-0.5 text-[10px]",
                      s.status === "ok" && "bg-emerald-500/15 text-emerald-300",
                      s.status === "degraded" && "bg-amber-500/15 text-amber-300",
                      s.status === "skip" && "bg-slate-700 text-slate-400",
                    )}
                  >
                    {s.status === "ok" ? "有效" : s.status === "degraded" ? "降级" : "跳过"}
                  </span>
                </div>
                <div className="flex items-center gap-3 text-xs">
                  <span className="text-slate-500">
                    权重 {s.weight_normalized.toFixed(0)}%
                  </span>
                  <span
                    className={cn(
                      "font-mono tabular-nums",
                      bullish ? "text-emerald-300" : "text-rose-300",
                    )}
                  >
                    {bullish ? "涨" : "跌"} {Math.max(s.up_probability, s.down_probability).toFixed(0)}%
                  </span>
                </div>
              </div>
              {s.note && (
                <p className="mt-1 text-[11px] leading-relaxed text-slate-500">{s.note}</p>
              )}
            </div>
          );
        })}
      </div>
    </motion.div>
  );
}
