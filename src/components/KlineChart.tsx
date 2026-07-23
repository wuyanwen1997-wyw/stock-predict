import { motion } from "framer-motion";
import { useMemo, useRef, useState, useEffect } from "react";
import { formatPrice } from "@/lib/utils";
import type { BsMarker, DailyBar } from "@/types";

interface Props {
  bars: DailyBar[];
  markers?: BsMarker[];
  loading?: boolean;
}

function formatDateLabel(date: string) {
  const parts = date.split("-");
  if (parts.length === 3) return `${parts[1]}-${parts[2]}`;
  return date;
}

function ChartTooltip({
  bar,
  marker,
  x,
  containerWidth,
}: {
  bar: DailyBar;
  marker?: BsMarker;
  x: number;
  containerWidth: number;
}) {
  const up = bar.close >= bar.open;
  const left = Math.min(Math.max(x - 70, 8), Math.max(containerWidth - 150, 8));

  return (
    <div
      className="pointer-events-none absolute top-2 z-10 rounded-lg border border-white/10 bg-slate-900/95 px-3 py-2 text-xs shadow-xl backdrop-blur-sm"
      style={{ left }}
    >
      <div className="text-slate-400">{bar.date}</div>
      <div className="mt-1 grid grid-cols-2 gap-x-4 gap-y-0.5 font-mono text-slate-200">
        <span className="text-slate-500">开</span>
        <span>¥{formatPrice(bar.open)}</span>
        <span className="text-slate-500">收</span>
        <span className={up ? "text-rose-400" : "text-emerald-400"}>
          ¥{formatPrice(bar.close)}
        </span>
        <span className="text-slate-500">高</span>
        <span>¥{formatPrice(bar.high)}</span>
        <span className="text-slate-500">低</span>
        <span>¥{formatPrice(bar.low)}</span>
      </div>
      {marker && (
        <div
          className={
            marker.kind === "buy"
              ? "mt-1.5 text-rose-400"
              : "mt-1.5 text-emerald-400"
          }
        >
          {marker.kind === "buy" ? "买入 B（MACD 金叉）" : "卖出 S（MACD 死叉）"}
        </div>
      )}
    </div>
  );
}

export function KlineChart({ bars, markers = [], loading }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const update = () => {
      const next = Math.floor(el.getBoundingClientRect().width);
      if (next > 0) setWidth(next);
    };

    update();
    requestAnimationFrame(update);

    const observer = new ResizeObserver(() => update());
    observer.observe(el);
    window.addEventListener("resize", update);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", update);
    };
  }, [loading, bars.length]);

  const latest = bars[bars.length - 1];
  const chartWidth = width > 0 ? width : 640;

  const markerByDate = useMemo(() => {
    const map = new Map<string, BsMarker>();
    for (const m of markers) map.set(m.date, m);
    return map;
  }, [markers]);

  const layout = useMemo(() => {
    if (bars.length === 0) return null;

    const height = 260;
    const pad = { top: 28, right: 12, bottom: 36, left: 52 };
    const plotW = Math.max(chartWidth - pad.left - pad.right, 1);
    const plotH = height - pad.top - pad.bottom;

    const min = Math.min(...bars.map((b) => b.low));
    const max = Math.max(...bars.map((b) => b.high));
    const span = Math.max(max - min, 0.01);
    const padY = span * 0.06;
    const yMin = min - padY;
    const yMax = max + padY;

    const yScale = (price: number) =>
      pad.top + ((yMax - price) / (yMax - yMin)) * plotH;

    const slot = plotW / bars.length;
    const bodyW = Math.max(2, Math.min(10, slot * 0.55));

    const candles = bars.map((bar, i) => {
      const cx = pad.left + slot * i + slot / 2;
      const yOpen = yScale(bar.open);
      const yClose = yScale(bar.close);
      const up = bar.close >= bar.open;
      const marker = markerByDate.get(bar.date);
      return {
        bar,
        cx,
        yHigh: yScale(bar.high),
        yLow: yScale(bar.low),
        bodyTop: Math.min(yOpen, yClose),
        bodyH: Math.max(Math.abs(yClose - yOpen), 1),
        up,
        color: up ? "#f87171" : "#34d399",
        marker,
      };
    });

    const ticks = 4;
    const yTicks = Array.from({ length: ticks + 1 }, (_, i) => {
      const price = yMin + ((yMax - yMin) * i) / ticks;
      return { price, y: yScale(price) };
    });

    const xTickStep = Math.max(1, Math.floor(bars.length / 6));
    const xTicks = bars
      .map((bar, i) => ({
        label: formatDateLabel(bar.date),
        x: pad.left + slot * i + slot / 2,
        i,
      }))
      .filter((t) => t.i % xTickStep === 0 || t.i === bars.length - 1);

    return { height, pad, candles, yTicks, xTicks, slot, bodyW };
  }, [bars, chartWidth, markerByDate]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      className="rounded-2xl border border-white/5 bg-slate-900/50 p-5 backdrop-blur-sm"
    >
      <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h3 className="text-base font-semibold text-slate-100">日 K 走势</h3>
          <p className="mt-1 text-xs text-slate-500">
            近 {bars.length} 个交易日 · 前复权日线
            {markers.length > 0 ? " · MACD 金叉/死叉 B/S" : ""}
          </p>
        </div>
        {latest && (
          <div className="text-right">
            <div className="font-mono text-lg font-bold tabular-nums text-slate-100">
              ¥{formatPrice(latest.close)}
            </div>
            <div className="text-xs text-slate-500">最新收盘 {latest.date}</div>
          </div>
        )}
      </div>

      {/* 始终存在，保证 ResizeObserver 能量到真实宽度 */}
      <div ref={containerRef} className="relative w-full min-h-64">
        {loading ? (
          <div className="animate-shimmer h-64 w-full rounded-xl" />
        ) : bars.length === 0 || !layout ? (
          <div className="flex h-64 items-center justify-center text-sm text-slate-500">
            暂无 K 线数据
          </div>
        ) : (
          <div
            className="relative w-full"
            onMouseLeave={() => setHoverIndex(null)}
          >
            {hoverIndex != null && layout.candles[hoverIndex] && (
              <ChartTooltip
                bar={layout.candles[hoverIndex].bar}
                marker={layout.candles[hoverIndex].marker}
                x={layout.candles[hoverIndex].cx}
                containerWidth={chartWidth}
              />
            )}

            <svg
              width="100%"
              height={layout.height}
              viewBox={`0 0 ${chartWidth} ${layout.height}`}
              preserveAspectRatio="xMidYMid meet"
              className="block w-full"
              onMouseMove={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                const ratio = chartWidth / Math.max(rect.width, 1);
                const x = (e.clientX - rect.left) * ratio;
                const idx = Math.min(
                  bars.length - 1,
                  Math.max(0, Math.floor((x - layout.pad.left) / layout.slot)),
                );
                if (x >= layout.pad.left && x <= chartWidth - layout.pad.right) {
                  setHoverIndex(idx);
                } else {
                  setHoverIndex(null);
                }
              }}
            >
              {layout.yTicks.map((tick) => (
                <g key={tick.price}>
                  <line
                    x1={layout.pad.left}
                    x2={chartWidth - layout.pad.right}
                    y1={tick.y}
                    y2={tick.y}
                    stroke="rgba(255,255,255,0.04)"
                    strokeDasharray="3 3"
                  />
                  <text
                    x={layout.pad.left - 8}
                    y={tick.y + 3}
                    textAnchor="end"
                    fill="#64748b"
                    fontSize={10}
                  >
                    {tick.price.toFixed(tick.price >= 100 ? 0 : 1)}
                  </text>
                </g>
              ))}

              {layout.xTicks.map((tick) => (
                <text
                  key={`${tick.label}-${tick.i}`}
                  x={tick.x}
                  y={layout.height - 8}
                  textAnchor="middle"
                  fill="#64748b"
                  fontSize={10}
                >
                  {tick.label}
                </text>
              ))}

              {layout.candles.map((c, i) => (
                <g key={c.bar.date}>
                  <line
                    x1={c.cx}
                    x2={c.cx}
                    y1={c.yHigh}
                    y2={c.yLow}
                    stroke={c.color}
                    strokeWidth={1}
                  />
                  <rect
                    x={c.cx - layout.bodyW / 2}
                    y={c.bodyTop}
                    width={layout.bodyW}
                    height={c.bodyH}
                    fill={c.color}
                    stroke={c.color}
                  />
                  {c.marker?.kind === "buy" && (
                    <text
                      x={c.cx}
                      y={c.yLow + 14}
                      textAnchor="middle"
                      fill="#f87171"
                      fontSize={11}
                      fontWeight={700}
                    >
                      B
                    </text>
                  )}
                  {c.marker?.kind === "sell" && (
                    <text
                      x={c.cx}
                      y={c.yHigh - 4}
                      textAnchor="middle"
                      fill="#34d399"
                      fontSize={11}
                      fontWeight={700}
                    >
                      S
                    </text>
                  )}
                  {hoverIndex === i && (
                    <line
                      x1={c.cx}
                      x2={c.cx}
                      y1={layout.pad.top}
                      y2={layout.height - layout.pad.bottom}
                      stroke="rgba(255,255,255,0.15)"
                      strokeDasharray="2 2"
                    />
                  )}
                </g>
              ))}
            </svg>
          </div>
        )}
      </div>
    </motion.div>
  );
}
