import { motion } from "framer-motion";
import type { PredictionResult } from "@/types";

interface Props {
  prediction: PredictionResult;
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

export function ProbabilityPanel({ prediction }: Props) {
  const { up_probability, down_probability, flat_probability, confidence } =
    prediction;

  return (
    <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-6 backdrop-blur-sm">
      <div className="mb-6 flex items-start justify-between">
        <div>
          <h2 className="text-lg font-semibold text-slate-100">明日涨跌概率</h2>
          <p className="mt-1 text-sm text-slate-500">
            预测日期 {prediction.predict_date}
          </p>
        </div>
        <div className="rounded-full border border-cyan-500/20 bg-cyan-500/10 px-3 py-1 text-xs text-cyan-300">
          置信度 {confidence.toFixed(1)}%
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
        <ArcGauge
          value={flat_probability}
          color="#94a3b8"
          label="平盘"
          delay={0.3}
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
            animate={{ width: `${flat_probability}%` }}
            transition={{ delay: 0.6, duration: 0.8 }}
            className="h-full bg-slate-500"
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
        <span>平 {flat_probability.toFixed(1)}%</span>
        <span>跌 {down_probability.toFixed(1)}%</span>
      </div>
    </div>
  );
}
