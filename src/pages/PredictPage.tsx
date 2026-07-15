import { motion } from "framer-motion";
import { BacktestPanel } from "@/components/BacktestPanel";
import { KlineChart } from "@/components/KlineChart";
import { ProbabilityPanel } from "@/components/ProbabilityPanel";
import { ScenarioChart } from "@/components/ScenarioChart";
import { SignalBreakdown } from "@/components/SignalBreakdown";
import { StockSelector } from "@/components/StockSelector";
import { StrategyComposer } from "@/components/StrategyComposer";
import { cn, formatPrice } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

export const LOOKBACK_OPTIONS = [25, 50, 60, 90, 120] as const;

export function PredictPage() {
  const selectedStock = useStockStore((s) => s.selectedStock);
  const prediction = useStockStore((s) => s.prediction);
  const klines = useStockStore((s) => s.klines);
  const backtest = useStockStore((s) => s.backtest);
  const predicting = useStockStore((s) => s.predicting);
  const loading = useStockStore((s) => s.loading);
  const loadingKlines = useStockStore((s) => s.loadingKlines);
  const loadingBacktest = useStockStore((s) => s.loadingBacktest);
  const lookbackDays = useStockStore((s) => s.lookbackDays);
  const setLookbackDays = useStockStore((s) => s.setLookbackDays);
  const runPrediction = useStockStore((s) => s.runPrediction);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="animate-shimmer h-32 w-64 rounded-2xl" />
      </div>
    );
  }

  return (
    <div className="p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -16 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-8"
      >
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="bg-gradient-to-r from-emerald-300 via-cyan-300 to-violet-300 bg-clip-text text-3xl font-bold text-transparent">
              智能预测
            </h1>
            <p className="mt-2 max-w-xl text-sm leading-relaxed text-slate-400">
              为每只股票组合技术面 / 消息面 / 政策面 / 美股等信号源，配置自动保存。
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-1.5 rounded-xl border border-white/5 bg-slate-900/50 p-1">
              <span className="px-2 text-xs text-slate-500">回看</span>
              {LOOKBACK_OPTIONS.map((days) => (
                <button
                  key={days}
                  type="button"
                  onClick={() => setLookbackDays(days)}
                  disabled={predicting}
                  className={cn(
                    "rounded-lg px-2.5 py-1.5 text-xs font-medium transition",
                    lookbackDays === days
                      ? "bg-cyan-500/20 text-cyan-300"
                      : "text-slate-400 hover:bg-white/5 hover:text-slate-200",
                    predicting && "opacity-50",
                  )}
                >
                  {days}日
                </button>
              ))}
            </div>

            {selectedStock && (
              <button
                type="button"
                onClick={() => void runPrediction()}
                disabled={predicting}
                className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-2 text-sm font-medium text-emerald-300 transition hover:bg-emerald-500/20 disabled:opacity-50"
              >
                {predicting ? "分析中..." : "重新预测"}
              </button>
            )}
          </div>
        </div>
      </motion.header>

      <div className="grid gap-6 xl:grid-cols-[340px_1fr]">
        <div className="space-y-4">
          <StockSelector />
          <StrategyComposer />
        </div>

        <div className="space-y-6">
          {predicting && !prediction ? (
            <div className="space-y-4">
              <div className="animate-shimmer h-64 rounded-2xl" />
              <div className="grid gap-4 md:grid-cols-2">
                <div className="animate-shimmer h-72 rounded-2xl" />
                <div className="animate-shimmer h-72 rounded-2xl" />
              </div>
            </div>
          ) : prediction ? (
            <>
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-wrap items-center gap-4 rounded-2xl border border-white/5 bg-gradient-to-r from-slate-900/80 to-slate-800/40 px-5 py-4 backdrop-blur-sm"
              >
                <div>
                  <div className="text-2xl font-bold text-slate-100">
                    {prediction.stock.name}
                  </div>
                  <div className="mt-0.5 text-sm text-slate-500">
                    {prediction.stock.market}.{prediction.stock.code} ·{" "}
                    {prediction.stock.sector} · 回看 {lookbackDays} 日 · 组合策略
                  </div>
                </div>
                <div className="ml-auto text-right">
                  <div className="text-xs text-slate-500">当前参考价</div>
                  <div className="font-mono text-2xl font-bold tabular-nums text-slate-100">
                    ¥{formatPrice(prediction.current_price)}
                  </div>
                </div>
              </motion.div>

              <ProbabilityPanel prediction={prediction} backtest={backtest} />

              <SignalBreakdown signals={prediction.signals ?? []} />

              <KlineChart bars={klines} loading={loadingKlines} />

              <BacktestPanel result={backtest} loading={loadingBacktest} />

              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: 0.2 }}
                className="rounded-xl border border-amber-500/15 bg-amber-500/5 px-4 py-3 text-sm leading-relaxed text-amber-200/80"
              >
                {prediction.summary}
              </motion.div>

              <div className="grid gap-6 lg:grid-cols-2">
                <ScenarioChart
                  scenario={prediction.high_open}
                  prevClose={prediction.current_price}
                  variant="high"
                  delay={0.3}
                />
                <ScenarioChart
                  scenario={prediction.low_open}
                  prevClose={prediction.current_price}
                  variant="low"
                  delay={0.45}
                />
              </div>
            </>
          ) : (
            <div className="flex h-64 items-center justify-center rounded-2xl border border-dashed border-white/10 text-slate-500">
              请从左侧选择一只股票开始预测
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
