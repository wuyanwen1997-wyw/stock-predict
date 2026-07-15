import { useEffect, useMemo, useState } from "react";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import type { StrategyCompose } from "@/types";

export function StrategyComposer() {
  const selectedStock = useStockStore((s) => s.selectedStock);
  const strategySources = useStockStore((s) => s.strategySources);
  const strategyMap = useStockStore((s) => s.strategyMap);
  const defaultCompose = useStockStore((s) => s.defaultCompose);
  const lookbackDays = useStockStore((s) => s.lookbackDays);
  const toggleSource = useStockStore((s) => s.toggleSource);
  const setSourceWeight = useStockStore((s) => s.setSourceWeight);
  const resetComposeForStock = useStockStore((s) => s.resetComposeForStock);
  const predicting = useStockStore((s) => s.predicting);

  const compose = useMemo((): StrategyCompose | null => {
    if (!selectedStock || !defaultCompose) return null;
    const saved = strategyMap[selectedStock.code];
    if (saved) {
      return {
        lookback_days: saved.lookback_days,
        sources: saved.sources.map((s) => ({ ...s })),
      };
    }
    return {
      lookback_days: lookbackDays || defaultCompose.lookback_days,
      sources: defaultCompose.sources.map((s) => ({ ...s })),
    };
  }, [selectedStock, strategyMap, defaultCompose, lookbackDays]);

  const [draftWeights, setDraftWeights] = useState<Record<string, number>>({});

  useEffect(() => {
    if (!compose) return;
    const next: Record<string, number> = {};
    for (const s of compose.sources) next[s.id] = s.weight;
    setDraftWeights(next);
  }, [selectedStock?.code, strategyMap]);

  if (!selectedStock || !compose || !defaultCompose) {
    return (
      <div className="rounded-2xl border border-dashed border-white/10 p-4 text-sm text-slate-500">
        选择股票后可配置预测组合
      </div>
    );
  }

  const infoMap = Object.fromEntries(strategySources.map((s) => [s.id, s]));

  return (
    <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-4 backdrop-blur-sm">
      <div className="mb-3 flex items-start justify-between gap-2">
        <div>
          <h2 className="text-sm font-medium text-slate-200">预测组合</h2>
          <p className="mt-0.5 text-xs text-slate-500">
            为 {selectedStock.name} 启用信号并设权重，配置按股票自动保存
          </p>
        </div>
        <button
          type="button"
          onClick={() => resetComposeForStock()}
          disabled={predicting}
          className="shrink-0 rounded-lg border border-white/10 px-2 py-1 text-xs text-slate-400 transition hover:bg-white/5 disabled:opacity-50"
        >
          重置
        </button>
      </div>

      <div className="max-h-[28rem] space-y-2 overflow-y-auto pr-1">
        {compose.sources.map((src) => {
          const info = infoMap[src.id];
          if (!info) return null;
          const weight = draftWeights[src.id] ?? src.weight;
          return (
            <div
              key={src.id}
              className={cn(
                "rounded-xl border px-3 py-2.5 transition",
                src.enabled
                  ? "border-cyan-500/25 bg-cyan-500/5"
                  : "border-white/5 bg-slate-800/30",
              )}
            >
              <div className="flex items-start gap-2">
                <button
                  type="button"
                  role="switch"
                  aria-checked={src.enabled}
                  disabled={predicting || !info.available}
                  onClick={() => toggleSource(src.id)}
                  className={cn(
                    "mt-0.5 h-5 w-9 shrink-0 rounded-full border transition",
                    src.enabled
                      ? "border-cyan-400/50 bg-cyan-500/40"
                      : "border-white/10 bg-slate-700/50",
                    predicting && "opacity-50",
                  )}
                >
                  <span
                    className={cn(
                      "block h-4 w-4 rounded-full bg-white transition",
                      src.enabled ? "translate-x-4" : "translate-x-0.5",
                    )}
                  />
                </button>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <span className="text-sm font-medium text-slate-200">{info.name}</span>
                    <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] text-slate-500">
                      {info.category}
                    </span>
                    {!info.backtestable && (
                      <span className="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300/80">
                        仅实时
                      </span>
                    )}
                  </div>
                  <p className="mt-0.5 text-[11px] leading-relaxed text-slate-500">
                    {info.description}
                  </p>
                  {src.enabled && (
                    <div className="mt-2 flex items-center gap-2">
                      <span className="text-[11px] text-slate-500">权重</span>
                      <input
                        type="range"
                        min={5}
                        max={100}
                        step={5}
                        value={weight}
                        disabled={predicting}
                        onChange={(e) =>
                          setDraftWeights((prev) => ({
                            ...prev,
                            [src.id]: Number(e.target.value),
                          }))
                        }
                        onMouseUp={() => setSourceWeight(src.id, weight)}
                        onTouchEnd={() => setSourceWeight(src.id, weight)}
                        className="h-1.5 flex-1 cursor-pointer accent-cyan-400"
                      />
                      <span className="w-8 text-right font-mono text-xs tabular-nums text-cyan-300">
                        {weight}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
