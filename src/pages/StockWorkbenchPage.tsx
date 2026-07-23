import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import { BacktestPanel } from "@/components/BacktestPanel";
import { KlineChart } from "@/components/KlineChart";
import { ProbabilityPanel } from "@/components/ProbabilityPanel";
import { ScenarioChart } from "@/components/ScenarioChart";
import { SignalBreakdown } from "@/components/SignalBreakdown";
import { StrategyComposer } from "@/components/StrategyComposer";
import { AddToPoolModal } from "@/components/AddToPoolModal";
import { cn, formatPct, formatPrice } from "@/lib/utils";
import { floatingPnl } from "@/lib/pool";
import { useStockStore } from "@/stores/stockStore";
import type { ChartDensity } from "@/types";
import {
  LOOKBACK_OPTIONS,
  TREND_HORIZON_OPTIONS,
} from "@/pages/PredictPage";

type Seg = "diagnose" | "chart" | "bs" | "scenario";

export function StockWorkbenchPage() {
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const seg = (params.get("tab") as Seg) || "diagnose";
  const setSeg = (t: Seg) => {
    const next = new URLSearchParams(params);
    next.set("tab", t);
    setParams(next, { replace: true });
  };

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
  const poolItems = useStockStore((s) => s.poolItems);
  const holdings = useStockStore((s) => s.holdings);
  const runPrediction = useStockStore((s) => s.runPrediction);
  const composeOpen = useStockStore((s) => s.composePanelOpen);
  const setComposeOpen = useStockStore((s) => s.setComposePanelOpen);

  const [poolOpen, setPoolOpen] = useState(false);
  const [density, setDensity] = useState<ChartDensity>(() =>
    typeof window !== "undefined" &&
    (window.innerWidth < 360 || window.innerHeight < 640)
      ? "compact"
      : "standard",
  );
  const immersive = params.get("immersive") === "1";

  const inPool = selectedStock
    ? poolItems.some((i) => i.code === selectedStock.code && i.groupId !== "g_holdings_mirror")
    : false;
  const holding = selectedStock
    ? holdings.find((h) => h.code === selectedStock.code)
    : undefined;
  const price = prediction?.current_price ?? selectedStock?.price;
  const pnl = holding ? floatingPnl(holding.cost, holding.qty, price) : null;

  const recentBs = useMemo(
    () => [...bsMarkers].slice(-8).reverse(),
    [bsMarkers],
  );

  useEffect(() => {
    if (immersive && seg !== "chart" && seg !== "bs") {
      setSeg("chart");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [immersive]);

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
          去行情选股
        </Link>
      </div>
    );
  }

  const tabs: { id: Seg; label: string }[] = [
    { id: "diagnose", label: "诊股" },
    { id: "chart", label: "行情" },
    { id: "bs", label: "买卖点" },
    { id: "scenario", label: "情景" },
  ];

  return (
    <div className="flex h-full min-h-0 flex-col">
      {!immersive && (
        <div className="shrink-0 border-b border-white/5 bg-slate-950/90 backdrop-blur-xl">
          <div className="flex flex-wrap items-center gap-2 px-3 py-2">
            <button
              type="button"
              onClick={() => navigate(-1)}
              className="text-slate-500"
            >
              ‹
            </button>
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium text-slate-100">
                {selectedStock.name}
                <span className="ml-1.5 font-mono text-[11px] text-slate-500">
                  {selectedStock.market}.{selectedStock.code}
                </span>
              </div>
              {price != null && (
                <div className="font-mono text-xs text-slate-400">
                  ¥{formatPrice(price)}
                  {selectedStock.change_pct != null && (
                    <span
                      className={cn(
                        "ml-2",
                        selectedStock.change_pct > 0 && "text-rose-400",
                        selectedStock.change_pct < 0 && "text-emerald-400",
                      )}
                    >
                      {formatPct(selectedStock.change_pct)}
                    </span>
                  )}
                </div>
              )}
            </div>
            <button
              type="button"
              onClick={() => setPoolOpen(true)}
              className={cn(
                "shrink-0 rounded-lg border px-2 py-1.5 text-[11px]",
                inPool
                  ? "border-amber-500/30 bg-amber-500/10 text-amber-300"
                  : "border-white/10 text-slate-400",
              )}
            >
              {inPool ? "已入池" : "入池"}
            </button>
            <Link to="/" className="text-[11px] text-cyan-400">
              换股
            </Link>
          </div>

          {holding && (
            <div className="flex items-center gap-3 border-t border-white/5 px-3 py-1.5 text-[11px] text-slate-400">
              <span>
                成本 ¥{formatPrice(holding.cost)} · {holding.qty} 股
              </span>
              {pnl && (
                <span
                  className={cn(
                    "font-mono",
                    pnl.pnl >= 0 ? "text-rose-400" : "text-emerald-400",
                  )}
                >
                  浮盈 {pnl.pnl >= 0 ? "+" : ""}
                  {pnl.pnl.toFixed(0)} ({formatPct(pnl.pct)})
                </span>
              )}
              <Link to="/holdings" className="ml-auto text-cyan-400">
                持仓
              </Link>
            </div>
          )}

          <div className="flex gap-1 overflow-x-auto px-2 pb-2 [scrollbar-width:none]">
            {tabs.map((t) => (
              <button
                key={t.id}
                type="button"
                onClick={() => setSeg(t.id)}
                className={cn(
                  "shrink-0 rounded-lg px-3 py-1.5 text-xs font-medium",
                  seg === t.id
                    ? "bg-emerald-500/20 text-emerald-300"
                    : "text-slate-500",
                )}
              >
                {t.label}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-3 sm:px-5">
        <div className={cn("mx-auto space-y-4", immersive ? "max-w-none" : "max-w-3xl")}>
          {seg === "diagnose" && (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <div className="flex items-center gap-0.5 rounded-lg border border-white/5 bg-slate-900/60 p-0.5">
                  <button
                    type="button"
                    onClick={() => setPredictMode("daily")}
                    className={cn(
                      "rounded-md px-2 py-1 text-[11px]",
                      predictMode === "daily"
                        ? "bg-emerald-500/20 text-emerald-300"
                        : "text-slate-500",
                    )}
                  >
                    单日
                  </button>
                  <button
                    type="button"
                    onClick={() => setPredictMode("trend")}
                    className={cn(
                      "rounded-md px-2 py-1 text-[11px]",
                      predictMode === "trend"
                        ? "bg-emerald-500/20 text-emerald-300"
                        : "text-slate-500",
                    )}
                  >
                    多日
                  </button>
                </div>
                {predictMode === "trend" &&
                  TREND_HORIZON_OPTIONS.map((d) => (
                    <button
                      key={d}
                      type="button"
                      onClick={() => setHorizonDays(d)}
                      className={cn(
                        "rounded-md px-2 py-1 text-[11px]",
                        horizonDays === d
                          ? "bg-violet-500/20 text-violet-300"
                          : "text-slate-500",
                      )}
                    >
                      {d}日
                    </button>
                  ))}
                {LOOKBACK_OPTIONS.map((d) => (
                  <button
                    key={d}
                    type="button"
                    onClick={() => setLookbackDays(d)}
                    className={cn(
                      "rounded-md px-2 py-1 text-[11px]",
                      lookbackDays === d
                        ? "bg-cyan-500/20 text-cyan-300"
                        : "text-slate-500",
                    )}
                  >
                    {d}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => void runPrediction()}
                  disabled={predicting}
                  className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1 text-[11px] text-emerald-300 disabled:opacity-50"
                >
                  {predicting ? "分析中" : "诊股"}
                </button>
                <button
                  type="button"
                  onClick={() => setComposeOpen(!composeOpen)}
                  className="ml-auto text-[11px] text-slate-400"
                >
                  信号组合 {composeOpen ? "▴" : "▾"}
                </button>
              </div>

              <AnimatePresence initial={false}>
                {composeOpen && (
                  <motion.div
                    initial={{ height: 0, opacity: 0 }}
                    animate={{ height: "auto", opacity: 1 }}
                    exit={{ height: 0, opacity: 0 }}
                    className="overflow-hidden rounded-xl border border-white/5 bg-slate-900/40"
                  >
                    <div className="max-h-56 overflow-y-auto p-2">
                      <StrategyComposer compact bare />
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {predicting && !prediction ? (
                <div className="animate-shimmer h-48 rounded-2xl" />
              ) : prediction ? (
                <>
                  <ProbabilityPanel prediction={prediction} backtest={backtest} />
                  <div className="rounded-xl border border-amber-500/15 bg-amber-500/5 px-4 py-3 text-sm leading-relaxed text-amber-200/80">
                    {prediction.summary}
                  </div>
                  <p className="text-[11px] text-slate-500">
                    预测与技术标记仅供研究演示，不构成投资建议。
                  </p>
                  <SignalBreakdown signals={prediction.signals ?? []} />
                  <button
                    type="button"
                    onClick={() => setSeg("chart")}
                    className="text-xs text-cyan-400"
                  >
                    查看行情图 ›
                  </button>
                  <BacktestPanel result={backtest} loading={loadingBacktest} />
                </>
              ) : (
                <div className="flex h-40 items-center justify-center rounded-2xl border border-dashed border-white/10 text-sm text-slate-500">
                  点击「诊股」开始分析
                </div>
              )}
            </>
          )}

          {(seg === "chart" || seg === "bs") && (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[11px] text-slate-500">密度</span>
                {(
                  [
                    ["compact", "精简"],
                    ["standard", "标准"],
                    ["full", "完整"],
                  ] as const
                ).map(([id, label]) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => setDensity(id)}
                    className={cn(
                      "rounded-md px-2 py-1 text-[11px]",
                      density === id
                        ? "bg-sky-500/20 text-sky-300"
                        : "text-slate-500",
                    )}
                  >
                    {label}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => {
                    const next = new URLSearchParams(params);
                    if (immersive) next.delete("immersive");
                    else next.set("immersive", "1");
                    setParams(next, { replace: true });
                  }}
                  className="ml-auto rounded-lg border border-white/10 px-2 py-1 text-[11px] text-slate-300"
                >
                  {immersive ? "退出全屏" : "全屏看图"}
                </button>
              </div>
              <KlineChart
                bars={klines}
                markers={seg === "bs" || klinePeriod === "day" ? bsMarkers : []}
                loading={loadingKlines}
                period={klinePeriod}
                onPeriodChange={setKlinePeriod}
                density={density}
                minHeight={immersive ? "min(70dvh, 720px)" : undefined}
              />
              {seg === "bs" && (
                <div className="rounded-xl border border-white/5 bg-slate-900/40 p-3">
                  <h3 className="text-sm font-medium text-slate-200">最近 B/S</h3>
                  <p className="mt-1 text-[11px] text-slate-500">
                    MACD 金叉/死叉研究口径，非下单指令。
                  </p>
                  {holding && pnl && (
                    <p className="mt-2 text-[11px] text-slate-400">
                      持仓浮盈参考：{pnl.pnl >= 0 ? "+" : ""}
                      {pnl.pnl.toFixed(0)} 元
                    </p>
                  )}
                  <ul className="mt-2 space-y-1 text-xs text-slate-300">
                    {recentBs.length === 0 && (
                      <li className="text-slate-500">暂无标记（需日 K）</li>
                    )}
                    {recentBs.map((m) => (
                      <li key={`${m.date}-${m.kind}`} className="flex gap-2">
                        <span
                          className={
                            m.kind === "buy" ? "text-rose-400" : "text-emerald-400"
                          }
                        >
                          {m.kind === "buy" ? "B" : "S"}
                        </span>
                        <span className="font-mono text-slate-400">{m.date}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          )}

          {seg === "scenario" && prediction && (
            <div className="grid gap-4 sm:grid-cols-2">
              <ScenarioChart
                scenario={prediction.high_open}
                prevClose={prediction.current_price}
                variant="high"
                delay={0.1}
              />
              <ScenarioChart
                scenario={prediction.low_open}
                prevClose={prediction.current_price}
                variant="low"
                delay={0.2}
              />
              {holding && pnl && (
                <p className="text-[11px] text-slate-500 sm:col-span-2">
                  情景仅示意路径；持仓浮盈按现价估算，不构成交易建议。
                </p>
              )}
            </div>
          )}
          {seg === "scenario" && !prediction && (
            <div className="text-sm text-slate-500">请先完成诊股</div>
          )}
        </div>
      </div>

      {poolOpen && selectedStock && (
        <AddToPoolModal stock={selectedStock} onClose={() => setPoolOpen(false)} />
      )}
    </div>
  );
}
