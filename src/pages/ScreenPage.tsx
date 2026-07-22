import { useMemo, useState } from "react";
import { motion } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { cn, formatPct, formatPrice, marketLabel } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import type { ScreenHit, ScreenUniverse } from "@/types";

const UNIVERSE_OPTIONS: { id: ScreenUniverse; label: string; hint: string }[] = [
  { id: "mixed", label: "综合池", hint: "人气榜∪自选∪种子" },
  { id: "hot", label: "人气榜", hint: "多源热股 Top50" },
  { id: "watchlist", label: "自选股", hint: "仅扫描自选" },
  { id: "seed", label: "种子池", hint: "内置 stocks.json" },
];

export function ScreenPage() {
  const navigate = useNavigate();
  const [composeOpen, setComposeOpen] = useState(false);

  const screenUniverse = useStockStore((s) => s.screenUniverse);
  const screenFilters = useStockStore((s) => s.screenFilters);
  const screenCompose = useStockStore((s) => s.screenCompose);
  const screenHorizonDays = useStockStore((s) => s.screenHorizonDays);
  const screenTopN = useStockStore((s) => s.screenTopN);
  const screenResult = useStockStore((s) => s.screenResult);
  const screening = useStockStore((s) => s.screening);
  const screenProgress = useStockStore((s) => s.screenProgress);
  const strategySources = useStockStore((s) => s.strategySources);
  const watchlist = useStockStore((s) => s.watchlist);
  const error = useStockStore((s) => s.error);

  const setScreenUniverse = useStockStore((s) => s.setScreenUniverse);
  const setScreenFilters = useStockStore((s) => s.setScreenFilters);
  const setScreenHorizonDays = useStockStore((s) => s.setScreenHorizonDays);
  const setScreenTopN = useStockStore((s) => s.setScreenTopN);
  const toggleScreenSource = useStockStore((s) => s.toggleScreenSource);
  const setScreenSourceWeight = useStockStore((s) => s.setScreenSourceWeight);
  const resetScreenCompose = useStockStore((s) => s.resetScreenCompose);
  const runSmartScreen = useStockStore((s) => s.runSmartScreen);
  const applyScreenHit = useStockStore((s) => s.applyScreenHit);
  const toggleWatchlist = useStockStore((s) => s.toggleWatchlist);

  const progressPct = useMemo(() => {
    if (!screenProgress.total) return 0;
    return Math.min(100, Math.round((screenProgress.done / screenProgress.total) * 100));
  }, [screenProgress]);

  const infoMap = useMemo(
    () => Object.fromEntries(strategySources.map((s) => [s.id, s])),
    [strategySources],
  );

  const goPredict = (hit: ScreenHit) => {
    applyScreenHit(hit);
    navigate("/predict");
  };

  const isWatched = (code: string) => watchlist.some((s) => s.code === code);

  return (
    <div className="h-full min-h-0 overflow-y-auto p-4 sm:p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -12 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-5"
      >
        <h1 className="text-xl font-semibold text-slate-100 sm:text-2xl">智能选股</h1>
        <p className="mt-1.5 text-sm text-slate-400">
          硬过滤 + 技术策略打分，结果可一键进入预测。仅供研究演示。
        </p>
      </motion.header>

      <section className="mb-4 space-y-3 rounded-2xl border border-white/5 bg-slate-900/50 p-3 sm:p-4">
        <div>
          <div className="mb-2 text-xs font-medium text-slate-400">股票池</div>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            {UNIVERSE_OPTIONS.map((opt) => (
              <button
                key={opt.id}
                type="button"
                disabled={screening}
                onClick={() => setScreenUniverse(opt.id)}
                className={cn(
                  "rounded-xl border px-2.5 py-2 text-left transition",
                  screenUniverse === opt.id
                    ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-200"
                    : "border-white/5 bg-slate-800/40 text-slate-300 hover:bg-slate-800/70",
                  screening && "opacity-50",
                )}
              >
                <div className="text-xs font-medium sm:text-sm">{opt.label}</div>
                <div className="mt-0.5 text-[10px] text-slate-500">{opt.hint}</div>
              </button>
            ))}
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          <label className="flex items-center gap-2 text-xs text-slate-300">
            <input
              type="checkbox"
              checked={screenFilters.exclude_st}
              disabled={screening}
              onChange={(e) => setScreenFilters({ exclude_st: e.target.checked })}
              className="rounded border-white/20"
            />
            排除 ST
          </label>
          <label className="flex items-center gap-2 text-xs text-slate-300">
            <input
              type="checkbox"
              checked={screenFilters.main_board_only}
              disabled={screening}
              onChange={(e) => setScreenFilters({ main_board_only: e.target.checked })}
              className="rounded border-white/20"
            />
            仅主板
          </label>
          <label className="flex flex-col gap-1 text-xs text-slate-400">
            最低价
            <input
              type="number"
              step="0.5"
              disabled={screening}
              value={screenFilters.min_price ?? ""}
              onChange={(e) =>
                setScreenFilters({
                  min_price: e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="rounded-lg border border-white/10 bg-slate-950/60 px-2 py-1.5 text-slate-200"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-slate-400">
            涨跌幅下限 %
            <input
              type="number"
              step="0.5"
              disabled={screening}
              value={screenFilters.min_change_pct ?? ""}
              onChange={(e) =>
                setScreenFilters({
                  min_change_pct: e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="rounded-lg border border-white/10 bg-slate-950/60 px-2 py-1.5 text-slate-200"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-slate-400">
            涨跌幅上限 %
            <input
              type="number"
              step="0.5"
              disabled={screening}
              value={screenFilters.max_change_pct ?? ""}
              onChange={(e) =>
                setScreenFilters({
                  max_change_pct: e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="rounded-lg border border-white/10 bg-slate-950/60 px-2 py-1.5 text-slate-200"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-slate-400">
            预测跨度（日）
            <select
              disabled={screening}
              value={screenHorizonDays}
              onChange={(e) => setScreenHorizonDays(Number(e.target.value))}
              className="rounded-lg border border-white/10 bg-slate-950/60 px-2 py-1.5 text-slate-200"
            >
              {[1, 2, 3, 4, 5].map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs text-slate-400">
            Top N
            <input
              type="number"
              min={5}
              max={50}
              disabled={screening}
              value={screenTopN}
              onChange={(e) => setScreenTopN(Number(e.target.value))}
              className="rounded-lg border border-white/10 bg-slate-950/60 px-2 py-1.5 text-slate-200"
            />
          </label>
        </div>

        <div>
          <button
            type="button"
            onClick={() => setComposeOpen((v) => !v)}
            className="text-xs text-cyan-300/90 hover:text-cyan-200"
          >
            {composeOpen ? "收起策略组合 ▴" : "展开策略组合 ▾"}
          </button>
          {composeOpen && screenCompose && (
            <div className="mt-2 max-h-64 space-y-1.5 overflow-y-auto pr-1">
              <div className="mb-1 flex justify-end">
                <button
                  type="button"
                  disabled={screening}
                  onClick={() => void resetScreenCompose()}
                  className="rounded-lg border border-white/10 px-2 py-1 text-[10px] text-slate-400 hover:bg-white/5"
                >
                  重置为选股默认
                </button>
              </div>
              {screenCompose.sources.map((src) => {
                const info = infoMap[src.id];
                const liveOnly = info && !info.backtestable;
                return (
                  <div
                    key={src.id}
                    className={cn(
                      "flex items-center gap-2 rounded-xl border px-2 py-1.5",
                      src.enabled
                        ? "border-cyan-500/25 bg-cyan-500/5"
                        : "border-white/5 bg-slate-800/30",
                    )}
                  >
                    <button
                      type="button"
                      role="switch"
                      aria-checked={src.enabled}
                      disabled={screening || liveOnly}
                      title={liveOnly ? "选股批量扫描不使用仅实时信号" : undefined}
                      onClick={() => toggleScreenSource(src.id)}
                      className={cn(
                        "h-4 w-7 shrink-0 rounded-full border transition",
                        src.enabled
                          ? "border-cyan-400/50 bg-cyan-500/40"
                          : "border-white/10 bg-slate-700/50",
                      )}
                    >
                      <span
                        className={cn(
                          "block h-3 w-3 rounded-full bg-white transition",
                          src.enabled ? "translate-x-3" : "translate-x-0.5",
                        )}
                      />
                    </button>
                    <span className="min-w-0 flex-1 truncate text-[11px] text-slate-200">
                      {info?.name ?? src.id}
                      {liveOnly && (
                        <span className="ml-1 text-[9px] text-amber-300/70">仅实时</span>
                      )}
                    </span>
                    {src.enabled && !liveOnly && (
                      <input
                        type="range"
                        min={5}
                        max={100}
                        step={5}
                        disabled={screening}
                        value={src.weight}
                        onChange={(e) =>
                          setScreenSourceWeight(src.id, Number(e.target.value))
                        }
                        className="w-20 accent-cyan-400"
                      />
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            disabled={screening}
            onClick={() => void runSmartScreen()}
            className={cn(
              "rounded-xl bg-gradient-to-r from-emerald-600 to-cyan-600 px-4 py-2 text-sm font-medium text-white shadow-lg shadow-emerald-900/30 transition",
              screening ? "opacity-60" : "hover:brightness-110",
            )}
          >
            {screening ? "选股中…" : "开始智能选股"}
          </button>
          {screening && (
            <div className="min-w-[10rem] flex-1">
              <div className="mb-1 flex justify-between text-[10px] text-slate-400">
                <span>
                  {screenProgress.done}/{screenProgress.total || "…"}
                  {screenProgress.code ? ` · ${screenProgress.code}` : ""}
                </span>
                <span>{progressPct}%</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-slate-800">
                <div
                  className="h-full rounded-full bg-emerald-400/80 transition-all"
                  style={{ width: `${progressPct}%` }}
                />
              </div>
            </div>
          )}
        </div>
      </section>

      {error && (
        <div className="mb-3 rounded-xl border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-xs text-rose-200">
          {error}
        </div>
      )}

      {screenResult && (
        <section>
          <p className="mb-3 text-xs text-slate-400">{screenResult.summary}</p>
          {screenResult.hits.length === 0 ? (
            <div className="flex h-32 items-center justify-center rounded-2xl border border-dashed border-white/10 text-sm text-slate-500">
              无入选标的，可放宽过滤条件后重试
            </div>
          ) : (
            <div className="space-y-2">
              {screenResult.hits.map((hit, i) => (
                <motion.div
                  key={hit.stock.code}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: Math.min(i * 0.03, 0.3) }}
                  className="flex items-stretch gap-2 rounded-2xl border border-white/5 bg-slate-900/50 p-3"
                >
                  <button
                    type="button"
                    onClick={() => goPredict(hit)}
                    className="min-w-0 flex-1 text-left"
                  >
                    <div className="flex items-start gap-2">
                      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800 text-[10px] font-bold text-slate-400">
                        {hit.stock.sector === "ETF"
                          ? "ETF"
                          : marketLabel(hit.stock.market)}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-baseline gap-2">
                          <span className="font-medium text-slate-100">{hit.stock.name}</span>
                          <span className="text-xs text-slate-500">{hit.stock.code}</span>
                          <span
                            className={cn(
                              "rounded px-1.5 py-0.5 text-[10px]",
                              hit.direction === "up" && "bg-rose-500/15 text-rose-300",
                              hit.direction === "down" && "bg-emerald-500/15 text-emerald-300",
                              hit.direction !== "up" &&
                                hit.direction !== "down" &&
                                "bg-slate-500/20 text-slate-400",
                            )}
                          >
                            {hit.direction === "up"
                              ? "看涨"
                              : hit.direction === "down"
                                ? "看跌"
                                : "中性"}
                          </span>
                        </div>
                        <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 font-mono text-xs tabular-nums text-slate-400">
                          {hit.stock.price != null && (
                            <span>¥{formatPrice(hit.stock.price)}</span>
                          )}
                          {hit.stock.change_pct != null && (
                            <span
                              className={cn(
                                hit.stock.change_pct > 0 && "text-rose-400",
                                hit.stock.change_pct < 0 && "text-emerald-400",
                              )}
                            >
                              {formatPct(hit.stock.change_pct)}
                            </span>
                          )}
                          <span className="text-cyan-300">
                            上涨 {hit.up_probability.toFixed(1)}%
                          </span>
                          <span>置信 {hit.confidence.toFixed(1)}</span>
                          <span>因子 {hit.factor_score.toFixed(2)}</span>
                        </div>
                        {hit.hints.length > 0 && (
                          <p className="mt-1 line-clamp-1 text-[10px] text-slate-500">
                            {hit.hints.slice(0, 2).join(" · ")}
                          </p>
                        )}
                      </div>
                    </div>
                  </button>
                  <div className="flex shrink-0 flex-col gap-1">
                    <button
                      type="button"
                      onClick={() => goPredict(hit)}
                      className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-2 py-1 text-[10px] text-emerald-200"
                    >
                      预测
                    </button>
                    <button
                      type="button"
                      onClick={() => toggleWatchlist(hit.stock)}
                      className="rounded-lg border border-white/10 px-2 py-1 text-[10px] text-slate-400"
                    >
                      {isWatched(hit.stock.code) ? "已自选" : "加自选"}
                    </button>
                  </div>
                </motion.div>
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  );
}
