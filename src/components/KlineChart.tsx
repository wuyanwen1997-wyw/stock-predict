import { motion } from "framer-motion";
import { useEffect, useRef } from "react";
import {
  dispose,
  init,
  registerOverlay,
  type Chart,
  type DeepPartial,
  type Styles,
} from "klinecharts";
import { cn, formatPrice } from "@/lib/utils";
import {
  dailyBarsToKLineData,
  KLINE_PERIOD_OPTIONS,
  periodSubtitle,
} from "@/lib/klineData";
import type { BsMarker, ChartDensity, DailyBar, KlinePeriod } from "@/types";

interface Props {
  bars: DailyBar[];
  markers?: BsMarker[];
  loading?: boolean;
  period: KlinePeriod;
  onPeriodChange: (period: KlinePeriod) => void;
  /** 窗格密度：精简 / 标准 / 完整 */
  density?: ChartDensity;
  /** 主图区域最小高度，如 max(280px, 45dvh) */
  minHeight?: string;
  compactHeader?: boolean;
}

const UP = "#f87171";
const DOWN = "#34d399";
const BS_OVERLAY = "bsMarker";
let bsOverlayRegistered = false;

function ensureBsOverlay() {
  if (bsOverlayRegistered) return;
  bsOverlayRegistered = true;
  registerOverlay({
    name: BS_OVERLAY,
    totalStep: 1,
    needDefaultPointFigure: false,
    createPointFigures: ({ coordinates, overlay }) => {
      const kind = overlay.extendData as "buy" | "sell";
      const isBuy = kind === "buy";
      const { x, y } = coordinates[0];
      return [
        {
          type: "text",
          attrs: {
            x,
            y: isBuy ? y + 12 : y - 4,
            text: isBuy ? "B" : "S",
            align: "center",
            baseline: isBuy ? "top" : "bottom",
          },
          styles: {
            color: isBuy ? UP : DOWN,
            size: 11,
            weight: 700,
          },
          ignoreEvent: true,
        },
      ];
    },
  });
}

function chartStyles(): DeepPartial<Styles> {
  return {
    grid: {
      horizontal: {
        color: "rgba(255,255,255,0.04)",
      },
      vertical: {
        color: "rgba(255,255,255,0.03)",
      },
    },
    candle: {
      bar: {
        upColor: UP,
        downColor: DOWN,
        noChangeColor: "#94a3b8",
        upBorderColor: UP,
        downBorderColor: DOWN,
        noChangeBorderColor: "#94a3b8",
        upWickColor: UP,
        downWickColor: DOWN,
        noChangeWickColor: "#94a3b8",
      },
      tooltip: {
        text: {
          color: "#e2e8f0",
        },
      },
      priceMark: {
        high: { color: "#94a3b8" },
        low: { color: "#94a3b8" },
        last: {
          upColor: UP,
          downColor: DOWN,
          noChangeColor: "#94a3b8",
        },
      },
    },
    indicator: {
      ohlc: {
        upColor: UP,
        downColor: DOWN,
        noChangeColor: "#94a3b8",
      },
      bars: [
        {
          upColor: "rgba(248,113,113,0.55)",
          downColor: "rgba(52,211,153,0.55)",
          noChangeColor: "rgba(148,163,184,0.45)",
        },
      ],
      tooltip: {
        text: { color: "#cbd5e1" },
      },
    },
    xAxis: {
      axisLine: { color: "rgba(255,255,255,0.08)" },
      tickLine: { color: "rgba(255,255,255,0.08)" },
      tickText: { color: "#64748b" },
    },
    yAxis: {
      axisLine: { color: "rgba(255,255,255,0.08)" },
      tickLine: { color: "rgba(255,255,255,0.08)" },
      tickText: { color: "#64748b" },
    },
    separator: {
      color: "rgba(255,255,255,0.08)",
      activeBackgroundColor: "rgba(148,163,184,0.25)",
    },
    crosshair: {
      horizontal: {
        line: { color: "rgba(255,255,255,0.2)" },
        text: { backgroundColor: "#1e293b", color: "#e2e8f0" },
      },
      vertical: {
        line: { color: "rgba(255,255,255,0.2)" },
        text: { backgroundColor: "#1e293b", color: "#e2e8f0" },
      },
    },
  };
}

function applyBsMarkers(chart: Chart, bars: DailyBar[], markers: BsMarker[]) {
  chart.removeOverlay({ name: BS_OVERLAY });
  if (markers.length === 0) return;

  const byDate = new Map(bars.map((b) => [b.date, b]));
  ensureBsOverlay();

  for (const m of markers) {
    const bar = byDate.get(m.date);
    if (!bar) continue;
    const data = dailyBarsToKLineData([bar])[0];
    if (!data) continue;
    chart.createOverlay({
      name: BS_OVERLAY,
      groupId: BS_OVERLAY,
      lock: true,
      points: [
        {
          timestamp: data.timestamp,
          value: m.kind === "buy" ? bar.low : bar.high,
        },
      ],
      extendData: m.kind,
    });
  }
}

export function KlineChart({
  bars,
  markers = [],
  loading,
  period,
  onPeriodChange,
  density = "standard",
  minHeight,
  compactHeader = false,
}: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<Chart | null>(null);
  const latest = bars[bars.length - 1];
  const showMarkers = period === "day" && markers.length > 0;
  const chartMinH = minHeight ?? "max(280px, 45dvh)";

  useEffect(() => {
    const el = hostRef.current;
    if (!el) return;

    ensureBsOverlay();
    const chart = init(el);
    if (!chart) return;
    chartRef.current = chart;

    chart.setTimezone("Asia/Shanghai");
    chart.setStyles(chartStyles());
    chart.setZoomEnabled(true);
    chart.setScrollEnabled(true);
    chart.setPriceVolumePrecision(2, 0);

    chart.createIndicator(
      { name: "MA", calcParams: [5, 10, 20], shortName: "MA" },
      true,
      { id: "candle_pane" },
    );
    if (density !== "compact") {
      chart.createIndicator("VOL", false, { height: 72, dragEnabled: true });
      chart.createIndicator("MACD", false, { height: 80, dragEnabled: true });
    } else {
      chart.createIndicator("MACD", false, { height: 64, dragEnabled: true });
    }
    if (density === "full") {
      chart.createIndicator("RSI", false, { height: 72, dragEnabled: true });
    }

    const ro = new ResizeObserver(() => {
      chart.resize();
    });
    ro.observe(el);

    return () => {
      ro.disconnect();
      dispose(chart);
      chartRef.current = null;
    };
  }, [density]);

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart || loading) return;

    const data = dailyBarsToKLineData(bars);
    chart.applyNewData(data, false, () => {
      if (period === "day") {
        applyBsMarkers(chart, bars, markers);
      } else {
        chart.removeOverlay({ name: BS_OVERLAY });
      }
    });
  }, [bars, markers, period, loading]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      className={cn(
        "border border-white/5 bg-slate-900/50 backdrop-blur-sm",
        compactHeader ? "rounded-xl p-3" : "rounded-2xl p-5",
      )}
    >
      {!compactHeader && (
        <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
          <div>
            <h3 className="text-base font-semibold text-slate-100">K 线形态</h3>
            <p className="mt-1 text-xs text-slate-500">
              {bars.length > 0 ? `${bars.length} 根 · ` : ""}
              {periodSubtitle(period)}
              {showMarkers ? " · MACD 金叉/死叉 B/S" : ""}
              {" · 滚轮/双指缩放"}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {latest && (
              <div className="mr-1 text-right">
                <div className="font-mono text-lg font-bold tabular-nums text-slate-100">
                  ¥{formatPrice(latest.close)}
                </div>
                <div className="text-xs text-slate-500">{latest.date}</div>
              </div>
            )}
            <button
              type="button"
              title="复位到最新"
              onClick={() => chartRef.current?.scrollToRealTime()}
              className="rounded-lg border border-white/10 px-2 py-1 text-[11px] text-slate-400 hover:bg-white/5 hover:text-slate-200"
            >
              复位
            </button>
          </div>
        </div>
      )}

      <div className="mb-3 flex flex-wrap gap-1">
        {KLINE_PERIOD_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onPeriodChange(opt.value)}
            className={cn(
              "rounded-md px-2 py-1 text-[11px] font-medium transition-colors",
              period === opt.value
                ? "bg-sky-500/20 text-sky-300"
                : "bg-white/5 text-slate-400 hover:bg-white/10 hover:text-slate-200",
            )}
          >
            {opt.label}
          </button>
        ))}
        {compactHeader && (
          <button
            type="button"
            onClick={() => chartRef.current?.scrollToRealTime()}
            className="ml-auto rounded-md px-2 py-1 text-[11px] text-slate-400"
          >
            复位
          </button>
        )}
      </div>

      <div className="relative w-full">
        {loading && (
          <div className="absolute inset-0 z-10 animate-shimmer rounded-xl bg-slate-800/40" />
        )}
        {!loading && bars.length === 0 && (
          <div className="absolute inset-0 z-10 flex items-center justify-center rounded-xl bg-slate-900/80 text-sm text-slate-500">
            暂无 K 线数据
          </div>
        )}
        <div
          ref={hostRef}
          className="w-full touch-none overscroll-contain"
          style={{ height: chartMinH, minHeight: 280 }}
        />
      </div>
    </motion.div>
  );
}
