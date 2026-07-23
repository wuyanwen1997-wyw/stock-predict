import { useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { KlineChart } from "@/components/KlineChart";
import { getStockKlines } from "@/services/api";
import { useStockStore } from "@/stores/stockStore";
import { stockFromPoolItem } from "@/lib/pool";
import { cn } from "@/lib/utils";
import type { DailyBar, KlinePeriod, Stock } from "@/types";

export function ComparePage() {
  const [params, setParams] = useSearchParams();
  const poolItems = useStockStore((s) => s.poolItems);
  const stocks = useStockStore((s) => s.stocks);
  const watchlist = useStockStore((s) => s.watchlist);

  const codes = useMemo(() => {
    const raw = params.get("codes") ?? "";
    const list = raw
      .split(",")
      .map((c) => c.trim())
      .filter(Boolean);
    if (list.length > 0) return list.slice(0, 4);
    const fromWatch = watchlist.slice(0, 4).map((s) => s.code);
    if (fromWatch.length > 0) return fromWatch;
    return poolItems
      .filter((i) => i.groupId === "g_watch")
      .slice(0, 4)
      .map((i) => i.code);
  }, [params, watchlist, poolItems]);

  const resolveStock = (code: string): Stock | null => {
    return (
      stocks.find((s) => s.code === code) ||
      watchlist.find((s) => s.code === code) ||
      (() => {
        const item = poolItems.find((i) => i.code === code);
        return item ? stockFromPoolItem(item) : null;
      })()
    );
  };

  const [wide, setWide] = useState(
    () => typeof window !== "undefined" && window.innerWidth >= 600,
  );
  const [slide, setSlide] = useState(0);
  const [period, setPeriod] = useState<KlinePeriod>("day");
  const [barsMap, setBarsMap] = useState<Record<string, DailyBar[]>>({});
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const onR = () => setWide(window.innerWidth >= 600);
    window.addEventListener("resize", onR);
    return () => window.removeEventListener("resize", onR);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      setLoading(true);
      const next: Record<string, DailyBar[]> = {};
      await Promise.all(
        codes.map(async (code) => {
          const stock = resolveStock(code);
          if (!stock) return;
          try {
            const bars = await getStockKlines(stock, 120, period);
            next[code] = bars;
          } catch {
            next[code] = [];
          }
        }),
      );
      if (!cancelled) {
        setBarsMap(next);
        setLoading(false);
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [codes.join(","), period]);

  const activeCode = codes[Math.min(slide, Math.max(0, codes.length - 1))];

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/5 px-3 py-2">
        <Link to="/pool" className="text-slate-400">
          ‹ 池
        </Link>
        <h1 className="text-sm font-semibold">多股同步</h1>
        <span className="text-[11px] text-slate-500">
          {wide ? "网格" : "左右滑"} · 最多 4 只
        </span>
      </div>

      {codes.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-sm text-slate-500">
          请先在股票池选择标的
          <Link to="/pool" className="text-cyan-400">
            去股票池
          </Link>
        </div>
      ) : wide ? (
        <div
          className={cn(
            "min-h-0 flex-1 overflow-y-auto p-2",
            codes.length <= 2 ? "grid grid-cols-1 gap-2 sm:grid-cols-2" : "grid grid-cols-2 gap-2",
          )}
        >
          {codes.map((code) => {
            const stock = resolveStock(code);
            return (
              <div key={code} className="min-h-0">
                <div className="mb-1 flex items-center justify-between px-1 text-xs">
                  <span className="text-slate-200">{stock?.name ?? code}</span>
                  <Link to={`/stock/${code}?tab=chart`} className="text-cyan-400">
                    个股
                  </Link>
                </div>
                <KlineChart
                  bars={barsMap[code] ?? []}
                  loading={loading}
                  period={period}
                  onPeriodChange={setPeriod}
                  density="compact"
                  compactHeader
                  minHeight="280px"
                />
              </div>
            );
          })}
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="flex shrink-0 gap-1 overflow-x-auto px-2 py-2">
            {codes.map((code, i) => (
              <button
                key={code}
                type="button"
                onClick={() => setSlide(i)}
                className={cn(
                  "shrink-0 rounded-full px-3 py-1 text-xs",
                  i === slide
                    ? "bg-emerald-500/20 text-emerald-300"
                    : "bg-white/5 text-slate-400",
                )}
              >
                {resolveStock(code)?.name ?? code}
              </button>
            ))}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
            {activeCode && (
              <>
                <div className="mb-1 flex justify-between text-xs">
                  <span>{resolveStock(activeCode)?.name}</span>
                  <Link to={`/stock/${activeCode}?tab=chart`} className="text-cyan-400">
                    个股 ›
                  </Link>
                </div>
                <KlineChart
                  bars={barsMap[activeCode] ?? []}
                  loading={loading}
                  period={period}
                  onPeriodChange={setPeriod}
                  density="standard"
                  minHeight="max(280px, 55dvh)"
                />
                <div className="mt-2 flex justify-center gap-3 text-xs text-slate-500">
                  <button
                    type="button"
                    disabled={slide <= 0}
                    onClick={() => setSlide((s) => Math.max(0, s - 1))}
                  >
                    ‹ 上一只
                  </button>
                  <button
                    type="button"
                    disabled={slide >= codes.length - 1}
                    onClick={() => setSlide((s) => Math.min(codes.length - 1, s + 1))}
                  >
                    下一只 ›
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}

      <div className="hidden">
        <button
          type="button"
          onClick={() => {
            const next = new URLSearchParams(params);
            next.set("codes", codes.join(","));
            setParams(next);
          }}
        />
      </div>
    </div>
  );
}
