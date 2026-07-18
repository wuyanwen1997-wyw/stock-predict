import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import type { BacktestResult, PredictionResult } from "@/types";

interface Props {
  prediction: PredictionResult;
  backtest?: BacktestResult | null;
}

function ArcGauge({
  value,
  color,
  label,
  delay,
}: {
  value: number;
  color: string;
  label: string;
  delay: number;
}) {
  const radius = 54;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - (value / 100) * circumference;

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.9 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ delay, duration: 0.5 }}
      className="flex flex-col items-center"
    >
      <div className="relative">
        <svg width="140" height="140" className="-rotate-90">
          <circle
            cx="70"
            cy="70"
            r={radius}
            fill="none"
            stroke="rgba(255,255,255,0.06)"
            strokeWidth="10"
          />
          <motion.circle
            cx="70"
            cy="70"
            r={radius}
            fill="none"
            stroke={color}
            strokeWidth="10"
            strokeLinecap="round"
            strokeDasharray={circumference}
            initial={{ strokeDashoffset: circumference }}
            animate={{ strokeDashoffset: offset }}
            transition={{ delay: delay + 0.2, duration: 1, ease: "easeOut" }}
            style={{ filter: `drop-shadow(0 0 8px ${color}66)` }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <motion.span
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: delay + 0.5 }}
            className="text-2xl font-bold tabular-nums"
            style={{ color }}
          >
            {value.toFixed(1)}%
          </motion.span>
        </div>
      </div>
      <span className="mt-2 text-sm text-slate-400">{label}</span>
    </motion.div>
  );
}

export function ProbabilityPanel({ prediction, backtest }: Props) {
  const {
    up_probability,
    down_probability,
    confidence,
    predicted,
    high_confidence,
    high_confidence_threshold,
    horizon_days,
  } = prediction;
  const horizon = horizon_days && horizon_days > 1 ? horizon_days : 1;
  const title =
    horizon <= 1
      ? "下一交易日涨跌概率"
      : `未来 ${horizon} 个交易日累计涨跌概率`;
  const subtitle =
    horizon <= 1
      ? `预测日期 ${prediction.predict_date} · 二分类（涨 / 跌）`
      : `截止 ${prediction.predict_date} · 区间累计涨跌二分类`;

  const bullish = predicted === "up";
  const leadProb = Math.max(up_probability, down_probability);

  return (
    <div className="space-y-4">
      {high_confidence && (
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          className={cn(
            "rounded-2xl border px-5 py-4",
            bullish
              ? "border-emerald-500/30 bg-emerald-500/10"
              : "border-rose-500/30 bg-rose-500/10",
          )}
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <div
                className={cn(
                  "text-lg font-semibold",
                  bullish ? "text-emerald-300" : "text-rose-300",
                )}
              >
                高置信{bullish ? "看涨" : "看跌"}
              </div>
              <p className="mt-1 text-xs text-slate-400">
                领先侧概率 {leadProb.toFixed(1)}% ≥ 阈值 {high_confidence_threshold.toFixed(0)}%
              </p>
            </div>
            <div className="text-right">
              {backtest && backtest.high_confidence_samples > 0 ? (
                <>
                  <div
                    className={cn(
                      "font-mono text-2xl font-bold tabular-nums",
                      bullish ? "text-emerald-300" : "text-rose-300",
                    )}
                  >
                    {backtest.high_confidence_accuracy.toFixed(1)}%
                  </div>
                  <div className="text-xs text-slate-500">
                    历史高置信准确率 · {backtest.high_confidence_samples} 次
                  </div>
                </>
              ) : (
                <div className="text-xs text-slate-500">历史高置信样本不足</div>
              )}
            </div>
          </div>
        </motion.div>
      )}

      <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-6 backdrop-blur-sm">
        <div className="mb-6 flex items-start justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold text-slate-100">{title}</h2>
            <p className="mt-1 text-sm text-slate-500">
              {subtitle}
              <span className="text-slate-600"> · 周末/节假日自动顺延</span>
            </p>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2">
            {high_confidence && (
              <span
                className={cn(
                  "rounded-full border px-3 py-1 text-xs",
                  bullish
                    ? "border-emerald-500/30 bg-emerald-500/15 text-emerald-300"
                    : "border-rose-500/30 bg-rose-500/15 text-rose-300",
                )}
              >
                高置信
              </span>
            )}
            <div className="rounded-full border border-cyan-500/20 bg-cyan-500/10 px-3 py-1 text-xs text-cyan-300">
              置信度 {confidence.toFixed(1)}%
            </div>
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-around gap-4">
          <ArcGauge
            value={up_probability}
            color="#34d399"
            label="上涨"
            delay={0}
          />
          <ArcGauge
            value={down_probability}
            color="#f87171"
            label="下跌"
            delay={0.15}
          />
        </div>

        <div className="mt-6 h-3 overflow-hidden rounded-full bg-slate-800">
          <div className="flex h-full">
            <motion.div
              initial={{ width: 0 }}
              animate={{ width: `${up_probability}%` }}
              transition={{ delay: 0.5, duration: 0.8 }}
              className="h-full bg-gradient-to-r from-emerald-500 to-emerald-400"
            />
            <motion.div
              initial={{ width: 0 }}
              animate={{ width: `${down_probability}%` }}
              transition={{ delay: 0.7, duration: 0.8 }}
              className="h-full bg-gradient-to-r from-red-400 to-red-500"
            />
          </div>
        </div>

        <div className="mt-3 flex justify-between text-xs text-slate-500">
          <span>涨 {up_probability.toFixed(1)}%</span>
          <span>跌 {down_probability.toFixed(1)}%</span>
        </div>
      </div>
    </div>
  );
}
