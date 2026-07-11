import { motion } from "framer-motion";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { cn, formatPct, formatPrice } from "@/lib/utils";
import type { ScenarioForecast } from "@/types";

interface Props {
  scenario: ScenarioForecast;
  prevClose: number;
  variant: "high" | "low";
  delay?: number;
}

const variantStyles = {
  high: {
    stroke: "#34d399",
    fill: "url(#highGradient)",
    badge: "bg-emerald-500/15 text-emerald-300 border-emerald-500/20",
    glow: "shadow-emerald-500/10",
  },
  low: {
    stroke: "#f87171",
    fill: "url(#lowGradient)",
    badge: "bg-red-500/15 text-red-300 border-red-500/20",
    glow: "shadow-red-500/10",
  },
};

function CustomTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: Array<{ payload: { time: string; price: number } }>;
}) {
  if (!active || !payload?.length) return null;
  const point = payload[0].payload;
  return (
    <div className="rounded-lg border border-white/10 bg-slate-900/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
      <div className="text-slate-400">{point.time}</div>
      <div className="mt-0.5 font-mono text-sm font-medium text-slate-100">
        ¥{formatPrice(point.price)}
      </div>
    </div>
  );
}

export function ScenarioChart({ scenario, prevClose, variant, delay = 0 }: Props) {
  const style = variantStyles[variant];
  const data = scenario.path.map((p) => ({ ...p }));

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay, duration: 0.5 }}
      className={cn(
        "rounded-2xl border border-white/5 bg-slate-900/50 p-5 backdrop-blur-sm shadow-lg",
        style.glow,
      )}
    >
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <h3 className="text-base font-semibold text-slate-100">
              {scenario.label}
            </h3>
            <span
              className={cn(
                "rounded-full border px-2 py-0.5 text-xs",
                style.badge,
              )}
            >
              开盘 ¥{formatPrice(scenario.open_price)}
            </span>
          </div>
          <p className="mt-1 text-xs text-slate-500">
            预测日内走势 · 18 个时间节点
          </p>
        </div>

        <div className="text-right">
          <div
            className={cn(
              "font-mono text-lg font-bold tabular-nums",
              scenario.change_pct >= 0 ? "text-emerald-400" : "text-red-400",
            )}
          >
            {formatPct(scenario.change_pct)}
          </div>
          <div className="text-xs text-slate-500">
            收盘 ¥{formatPrice(scenario.close_price)}
          </div>
        </div>
      </div>

      <div className="mb-3 grid grid-cols-3 gap-2 text-center text-xs">
        <div className="rounded-lg bg-slate-800/50 px-2 py-1.5">
          <div className="text-slate-500">最高</div>
          <div className="font-mono text-slate-200">
            ¥{formatPrice(scenario.high_price)}
          </div>
        </div>
        <div className="rounded-lg bg-slate-800/50 px-2 py-1.5">
          <div className="text-slate-500">最低</div>
          <div className="font-mono text-slate-200">
            ¥{formatPrice(scenario.low_price)}
          </div>
        </div>
        <div className="rounded-lg bg-slate-800/50 px-2 py-1.5">
          <div className="text-slate-500">昨收</div>
          <div className="font-mono text-slate-200">
            ¥{formatPrice(prevClose)}
          </div>
        </div>
      </div>

      <div className="h-52 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
            <defs>
              <linearGradient id="highGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#34d399" stopOpacity={0.35} />
                <stop offset="100%" stopColor="#34d399" stopOpacity={0} />
              </linearGradient>
              <linearGradient id="lowGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#f87171" stopOpacity={0.35} />
                <stop offset="100%" stopColor="#f87171" stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.04)" />
            <XAxis
              dataKey="time"
              tick={{ fill: "#64748b", fontSize: 10 }}
              axisLine={false}
              tickLine={false}
              interval={2}
            />
            <YAxis
              domain={["auto", "auto"]}
              tick={{ fill: "#64748b", fontSize: 10 }}
              axisLine={false}
              tickLine={false}
              width={48}
              tickFormatter={(v: number) => v.toFixed(1)}
            />
            <ReferenceLine
              y={prevClose}
              stroke="#64748b"
              strokeDasharray="4 4"
              label={{ value: "昨收", fill: "#64748b", fontSize: 10, position: "insideTopLeft" }}
            />
            <Tooltip content={<CustomTooltip />} />
            <Area
              type="monotone"
              dataKey="price"
              stroke={style.stroke}
              strokeWidth={2}
              fill={style.fill}
              animationDuration={1200}
              animationBegin={delay * 1000 + 200}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </motion.div>
  );
}
