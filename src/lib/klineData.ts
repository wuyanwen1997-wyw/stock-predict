import type { DailyBar, KlinePeriod } from "@/types";
import type { KLineData } from "klinecharts";

export const KLINE_PERIOD_OPTIONS: { value: KlinePeriod; label: string }[] = [
  { value: "min1", label: "1分" },
  { value: "min5", label: "5分" },
  { value: "min15", label: "15分" },
  { value: "min30", label: "30分" },
  { value: "min60", label: "60分" },
  { value: "day", label: "日" },
  { value: "week", label: "周" },
  { value: "month", label: "月" },
];

/** Parse DailyBar.date → local ms timestamp. */
export function barDateToTimestamp(date: string): number {
  const m = date
    .trim()
    .match(/^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2}))?)?/);
  if (!m) {
    const t = Date.parse(date);
    return Number.isFinite(t) ? t : 0;
  }
  const y = Number(m[1]);
  const mo = Number(m[2]) - 1;
  const d = Number(m[3]);
  const h = Number(m[4] ?? 0);
  const mi = Number(m[5] ?? 0);
  const s = Number(m[6] ?? 0);
  return new Date(y, mo, d, h, mi, s).getTime();
}

export function dailyBarsToKLineData(bars: DailyBar[]): KLineData[] {
  return bars
    .map((b) => ({
      timestamp: barDateToTimestamp(b.date),
      open: b.open,
      high: b.high,
      low: b.low,
      close: b.close,
      volume: b.volume,
    }))
    .filter((b) => b.timestamp > 0)
    .sort((a, b) => a.timestamp - b.timestamp);
}

export function chartBarLimit(period: KlinePeriod, lookbackDays: number): number {
  switch (period) {
    case "day":
      return Math.max(120, lookbackDays + 30);
    case "week":
      return 104;
    case "month":
      return 60;
    case "min1":
    case "min5":
      return 240;
    case "min15":
      return 160;
    case "min30":
    case "min60":
      return 120;
    default:
      return 120;
  }
}

export function periodSubtitle(period: KlinePeriod): string {
  switch (period) {
    case "day":
      return "前复权日线";
    case "week":
      return "前复权周线";
    case "month":
      return "前复权月线";
    case "min1":
      return "1 分钟 K";
    case "min5":
      return "5 分钟 K";
    case "min15":
      return "15 分钟 K";
    case "min30":
      return "30 分钟 K";
    case "min60":
      return "60 分钟 K";
    default:
      return "K 线";
  }
}
