import { useRef, type PointerEvent as ReactPointerEvent } from "react";
import { Link } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { BacktestPanel } from "@/components/BacktestPanel";
import { KlineChart } from "@/components/KlineChart";
import { ProbabilityPanel } from "@/components/ProbabilityPanel";
import { ScenarioChart } from "@/components/ScenarioChart";
import { SignalBreakdown } from "@/components/SignalBreakdown";
import { StrategyComposer } from "@/components/StrategyComposer";
import { cn, formatPrice } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

export const LOOKBACK_OPTIONS = [25, 50, 60, 90, 120] as const;
export const TREND_HORIZON_OPTIONS = [2, 3, 4, 5] as const;

/** Approx height of one compact compose row (with weight stepper). */
const ROW_H = 58;
const DEFAULT_VISIBLE = 4;
const MIN_H = ROW_H * 2;
const MAX_H = ROW_H * 8;
const DEFAULT_H = ROW_H * DEFAULT_VISIBLE;

export function PredictPage() {
  const selectedStock = useStockStore((s) => s.selectedStock);
  const prediction = useStockStore((s) => s.prediction);
  const klines = useStockStore((s) => s.klines);
  const klinePeriod = useStockStore((s) => s.klinePeriod);
  const setKlinePeriod = useStockStore((s) => s.setKlinePeriod);
  const bsMarkers = useStockStore((s) => s.bsMarkers);
  const backtest = useStockStore((s) => s.backtest);
  const predicting = useStockStore((s) => s.predicting);
  const loading = useStockStore((s) => s.loading);
  const loadingKlines = useStockStore((s) => s.loadingKlines);
  const loadingBacktest = useStockStore((s) => s.loadingBacktest);
  const lookbackDays = useStockStore((s) => s.lookbackDays);
  const setLookbackDays = useStockStore((s) => s.setLookbackDays);
  const predictMode = useStockStore((s) => s.predictMode);
  const setPredictMode = useStockStore((s) => s.setPredictMode);
  const horizonDays = useStockStore((s) => s.horizonDays);
  const setHorizonDays = useStockStore((s) => s.setHorizonDays);
  const watchlist = useStockStore((s) => s.watchlist);
  const toggleWatchlist = useStockStore((s) => s.toggleWatchlist);
  const composeOpen = useStockStore((s) => s.composePanelOpen);
  const setComposeOpen = useStockStore((s) => s.setComposePanelOpen);
  const composeHeight = useStockStore((s) => s.composePanelHeight);
  const setComposeHeight = useStockStore((s) => s.setComposePanelHeight);
  const runPrediction = useStockStore((s) => s.runPrediction);

  const starred = selectedStock
    ? watchlist.some((s) => s.code === selectedStock.code)
    : false;

  const resizeRef = useRef<{ startY: number; startH: number; pointerId: number } | null>(null);

  const onResizeDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    resizeRef.current = {
      pointerId: e.pointerId,
      startY: e.clientY,
      startH: composeHeight,
    };
  };

  const onResizeMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const drag = resizeRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    const next = Math.min(MAX_H, Math.max(MIN_H, drag.startH + (e.clientY - drag.startY)));
    setComposeHeight(next);
  };

  const onResizeUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const drag = resizeRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
    resizeRef.current = null;
  };

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="animate-shimmer h-32 w-64 rounded-2xl" />
      </div>
    );
  }

  if (!selectedStock) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-8 text-center">
        <p className="text-slate-400">还没有选中股票</p>
        <Link
          to="/"
          className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-300"
        >
          去首页选股
        </Link>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Fixed top: toolbar + collapsible compose */}
      <div className="shrink-0 border-b border-white/5 bg-slate-950/90 backdrop-blur-xl">
        <div className="flex flex-wrap items-center gap-2 px-3 py-2">
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium text-slate-100">
              {selectedStock.name}
              <span className="ml-1.5 font-mono text-[11px] text-slate-500">
                {selectedStock.market}.{selectedStock.code}
              </span>
            </div>
          </div>
          <button
            type="button"
            onClick={() => toggleWatchlist(selectedStock)}
            className={cn(
              "shrink-0 rounded-lg border px-2 py-1.5 text-sm transition",
              starred
                ? "border-amber-500/30 bg-amber-500/10 text-amber-300"
                : "border-white/10 text-slate-500 active:text-amber-300",
            )}
            aria-label={starred ? "取消自选" : "加入自选"}
          >
            {starred ? "★ 自选" : "☆ 自选"}
          </button>
          <Link to="/" className="text-[11px] text-cyan-400 hover:underline">
            换股
          </Link>
          <div className="flex max-w-full items-center gap-0.5 overflow-x-auto rounded-lg border border-white/5 bg-slate-900/60 p-0.5">
            <button
              type="button"
              onClick={() => setPredictMode("daily")}
              disabled={predicting}
              className={cn(
                "shrink-0 rounded-md px-2 py-1 text-[11px] font-medium transition",
                predictMode === "daily"
                  ? "bg-emerald-500/20 text-emerald-300"
                  : "text-slate-500",
                predicting && "opacity-50",
              )}
            >
              单日
            </button>
            <button
              type="button"
              onClick={() => setPredictMode("trend")}
              disabled={predicting}
              className={cn(
                "shrink-0 rounded-md px-2 py-1 text-[11px] font-medium transition",
                predictMode === "trend"
                  ? "bg-emerald-500/20 text-emerald-300"
                  : "text-slate-500",
                predicting && "opacity-50",
              )}
            >
              多日
            </button>
          </div>
          {predictMode === "trend" && (
            <div className="flex max-w-full items-center gap-0.5 overflow-x-auto rounded-lg border border-white/5 bg-slate-900/60 p-0.5">
              {TREND_HORIZON_OPTIONS.map((days) => (
                <button
                  key={days}
                  type="button"
                  onClick={() => setHorizonDays(days)}
                  disabled={predicting}
                  className={cn(
                    "shrink-0 rounded-md px-2 py-1 text-[11px] font-medium transition",
                    horizonDays === days
                      ? "bg-violet-500/20 text-violet-300"
                      : "text-slate-500",
                    predicting && "opacity-50",
                  )}
                >
                  {days}日
                </button>
              ))}
            </div>
          )}
          <div className="flex max-w-full items-center gap-0.5 overflow-x-auto rounded-lg border border-white/5 bg-slate-900/60 p-0.5">
            {LOOKBACK_OPTIONS.map((days) => (
              <button
                key={days}
                type="button"
                onClick={() => setLookbackDays(days)}
                disabled={predicting}
                className={cn(
                  "shrink-0 rounded-md px-2 py-1 text-[11px] font-medium transition",
                  lookbackDays === days
                    ? "bg-cyan-500/20 text-cyan-300"
                    : "text-slate-500",
                  predicting && "opacity-50",
                )}
              >
                {days}
              </button>
            ))}
          </div>
          <button
            type="button"
            onClick={() => void runPrediction()}
            disabled={predicting}
            className="shrink-0 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1.5 text-[11px] font-medium text-emerald-300 disabled:opacity-50"
          >
            {predicting ? "分析中" : "预测"}
          </button>
        </div>

        <div className="flex items-center gap-2 border-t border-white/5 px-3 py-1.5">
          <button
            type="button"
            onClick={() => setComposeOpen(!composeOpen)}
            className="flex min-w-0 flex-1 items-center gap-2 text-left"
          >
            <span className="text-xs font-medium text-slate-200">信号组合</span>
            <span className="truncate text-[10px] text-slate-500">
              默认约 4 条 · 拖底边调高度
            </span>
            <span className="ml-auto shrink-0 text-[10px] text-slate-400">
              {composeOpen ? "收起 ▴" : "展开 ▾"}
            </span>
          </button>
          {composeOpen && (
            <div className="flex shrink-0 gap-1">
              <button
                type="button"
                title="显示约 2 条"
                onClick={() => setComposeHeight(ROW_H * 2)}
                className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-slate-400"
              >
                矮
              </button>
              <button
                type="button"
                title="显示约 4 条"
                onClick={() => setComposeHeight(DEFAULT_H)}
                className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-slate-400"
              >
                中
              </button>
              <button
                type="button"
                title="显示约 6 条"
                onClick={() => setComposeHeight(ROW_H * 6)}
                className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-slate-400"
              >
                高
              </button>
            </div>
          )}
        </div>

        <AnimatePresence initial={false}>
          {composeOpen && (
            <motion.div
              key="compose"
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.18 }}
              className="overflow-hidden border-t border-white/5"
            >
              <div
                data-compose-scroll
                className="overflow-y-auto overscroll-contain px-2 pr-3 pt-1"
                style={{ height: composeHeight }}
              >
                <StrategyComposer compact bare />
              </div>
              <div
                onPointerDown={onResizeDown}
                onPointerMove={onResizeMove}
                onPointerUp={onResizeUp}
                onPointerCancel={onResizeUp}
                className="flex h-5 cursor-ns-resize touch-none items-center justify-center border-t border-white/5 bg-slate-950/60"
                title="上下拖动调整高度"
              >
                <div className="h-1 w-10 rounded-full bg-white/20" />
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Middle: scrollable prediction results */}
      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-4 sm:px-6">
        <div className="mx-auto max-w-3xl space-y-5">
          {predicting && !prediction ? (
            <div className="space-y-4">
              <div className="animate-shimmer h-48 rounded-2xl" />
              <div className="animate-shimmer h-56 rounded-2xl" />
            </div>
          ) : prediction ? (
            <>
              <div className="flex flex-wrap items-center gap-3 rounded-2xl border border-white/5 bg-gradient-to-r from-slate-900/80 to-slate-800/40 px-4 py-3">
                <div className="min-w-0">
                  <div className="text-lg font-bold text-slate-100">{prediction.stock.name}</div>
                  <div className="mt-0.5 text-xs text-slate-500">
                    {prediction.stock.market}.{prediction.stock.code} · {prediction.stock.sector} ·
                    回看 {lookbackDays} 日
                  </div>
                </div>
                <div className="ml-auto text-right">
                  <div className="text-[10px] text-slate-500">参考价</div>
                  <div className="font-mono text-xl font-bold tabular-nums text-slate-100">
                    ¥{formatPrice(prediction.current_price)}
                  </div>
                </div>
              </div>

              <ProbabilityPanel prediction={prediction} backtest={backtest} />
              <SignalBreakdown signals={prediction.signals ?? []} />
              <KlineChart
                bars={klines}
                markers={bsMarkers}
                loading={loadingKlines}
                period={klinePeriod}
                onPeriodChange={setKlinePeriod}
              />
              <BacktestPanel result={backtest} loading={loadingBacktest} />

              <div className="rounded-xl border border-amber-500/15 bg-amber-500/5 px-4 py-3 text-sm leading-relaxed text-amber-200/80">
                {prediction.summary}
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <ScenarioChart
                  scenario={prediction.high_open}
                  prevClose={prediction.current_price}
                  variant="high"
                  delay={0.2}
                />
                <ScenarioChart
                  scenario={prediction.low_open}
                  prevClose={prediction.current_price}
                  variant="low"
                  delay={0.3}
                />
              </div>
            </>
          ) : (
            <div className="flex h-48 items-center justify-center rounded-2xl border border-dashed border-white/10 text-sm text-slate-500">
              点击上方「预测」开始分析
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
