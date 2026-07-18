import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import type { StrategyCompose } from "@/types";

function clampWeight(n: number) {
  return Math.min(100, Math.max(5, Math.round(n / 5) * 5));
}

/**
 * Horizontal weight slider that ignores vertical pans (scroll) until
 * the gesture is clearly left/right.
 */
function WeightSlider({
  value,
  disabled,
  compact,
  onDraft,
  onCommit,
}: {
  value: number;
  disabled?: boolean;
  compact?: boolean;
  onDraft: (v: number) => void;
  onCommit: (v: number) => void;
}) {
  const trackRef = useRef<HTMLDivElement>(null);
  const gesture = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    mode: "undecided" | "horizontal" | "vertical";
  } | null>(null);

  const valueFromClientX = (clientX: number) => {
    const el = trackRef.current;
    if (!el) return value;
    const rect = el.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / Math.max(rect.width, 1)));
    return clampWeight(5 + ratio * 95);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (disabled) return;
    // 触控时锁定手势方向；鼠标保持 pan-y 以便滚轮滚动列表
    if (e.pointerType === "touch" || e.pointerType === "pen") {
      e.currentTarget.style.touchAction = "none";
    }
    gesture.current = {
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      mode: "undecided",
    };
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const g = gesture.current;
    if (!g || g.pointerId !== e.pointerId) return;

    const dx = e.clientX - g.startX;
    const dy = e.clientY - g.startY;

    if (g.mode === "undecided") {
      if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return;
      if (Math.abs(dx) > Math.abs(dy) * 1.15) {
        g.mode = "horizontal";
        e.currentTarget.setPointerCapture(e.pointerId);
      } else {
        g.mode = "vertical";
        // Let parent list scroll; abandon slider gesture
        gesture.current = null;
        return;
      }
    }

    if (g.mode === "horizontal") {
      e.preventDefault();
      // Preview only while dragging; commit on release
      onDraft(valueFromClientX(e.clientX));
    }
  };

  const end = (e: ReactPointerEvent<HTMLDivElement>) => {
    const g = gesture.current;
    e.currentTarget.style.touchAction = "pan-y";
    if (!g || g.pointerId !== e.pointerId) return;

    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }

    // Tap / click, or finished horizontal drag → apply on release
    if (g.mode === "horizontal" || g.mode === "undecided") {
      const next = valueFromClientX(e.clientX);
      onDraft(next);
      onCommit(next);
    }

    gesture.current = null;
  };

  const pct = ((value - 5) / 95) * 100;

  return (
    <div
      className={cn(
        "flex items-center gap-2",
        compact ? "mt-1.5" : "mt-2",
      )}
    >
      <span className={cn("shrink-0 text-slate-500", compact ? "text-[10px]" : "text-[11px]")}>
        权重
      </span>
      <div
        ref={trackRef}
        role="slider"
        aria-valuemin={5}
        aria-valuemax={100}
        aria-valuenow={value}
        tabIndex={disabled ? -1 : 0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={end}
        onPointerCancel={end}
        onWheel={(e) => {
          // 滚轮交给外层信号列表，避免被滑动条截获
          const scroller = e.currentTarget.closest("[data-compose-scroll]");
          if (scroller instanceof HTMLElement) {
            scroller.scrollTop += e.deltaY;
            e.preventDefault();
          }
        }}
        onKeyDown={(e) => {
          if (disabled) return;
          if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
            e.preventDefault();
            const next = clampWeight(value - 5);
            onDraft(next);
            onCommit(next);
          } else if (e.key === "ArrowRight" || e.key === "ArrowUp") {
            e.preventDefault();
            const next = clampWeight(value + 5);
            onDraft(next);
            onCommit(next);
          }
        }}
        className={cn(
          // 桌面限制宽度，避免 Windows 宽屏上滑条拉满整行
          "relative min-w-0 select-none py-2",
          compact ? "w-full max-w-[9.5rem] sm:max-w-[11rem]" : "w-full max-w-[12rem] sm:max-w-[14rem]",
          disabled && "opacity-50",
        )}
        style={{ touchAction: "pan-y" }}
      >
        <div className="relative h-1.5 rounded-full bg-slate-700/80">
          <div
            className="absolute inset-y-0 left-0 rounded-full bg-cyan-400/70"
            style={{ width: `${pct}%` }}
          />
          <div
            className="absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border border-cyan-200/80 bg-cyan-300 shadow"
            style={{ left: `${pct}%` }}
          />
        </div>
      </div>
      <span
        className={cn(
          "shrink-0 text-right font-mono tabular-nums text-cyan-300",
          compact ? "w-7 text-[10px]" : "w-8 text-xs",
        )}
      >
        {value}
      </span>
    </div>
  );
}

export function StrategyComposer({
  compact = false,
  bare = false,
}: {
  compact?: boolean;
  bare?: boolean;
}) {
  const selectedStock = useStockStore((s) => s.selectedStock);
  const strategySources = useStockStore((s) => s.strategySources);
  const strategyMap = useStockStore((s) => s.strategyMap);
  const defaultCompose = useStockStore((s) => s.defaultCompose);
  const lookbackDays = useStockStore((s) => s.lookbackDays);
  const getComposeForStock = useStockStore((s) => s.getComposeForStock);
  const toggleSource = useStockStore((s) => s.toggleSource);
  const setSourceWeight = useStockStore((s) => s.setSourceWeight);
  const resetComposeForStock = useStockStore((s) => s.resetComposeForStock);
  const applyTunedComposeForStock = useStockStore((s) => s.applyTunedComposeForStock);
  const predicting = useStockStore((s) => s.predicting);

  const compose = useMemo((): StrategyCompose | null => {
    if (!selectedStock || !defaultCompose) return null;
    return getComposeForStock(selectedStock.code);
  }, [selectedStock, strategyMap, defaultCompose, lookbackDays, getComposeForStock]);

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

  const body = (
    <>
      <div className={cn("mb-2 flex items-start justify-between gap-2", !compact && !bare && "mb-3")}>
        <div className="min-w-0">
          {!bare && (
            <h2 className={cn("font-medium text-slate-200", compact ? "text-xs sm:text-sm" : "text-sm")}>
              信号组合
            </h2>
          )}
          {!compact && !bare && (
            <p className="mt-0.5 text-xs text-slate-500">
              为 {selectedStock.name} 启用信号并设权重，配置按股票自动保存
            </p>
          )}
          {strategySources.length === 0 && (
            <p className="mt-1 text-[10px] text-amber-300/80 sm:text-xs">信号源说明未加载</p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button
            type="button"
            onClick={() => applyTunedComposeForStock()}
            disabled={predicting}
            title="宽基推荐：技术多因子 70% + 消息面 30%"
            className="rounded-lg border border-cyan-500/30 bg-cyan-500/10 px-2 py-1 text-[10px] text-cyan-200 transition hover:bg-cyan-500/20 disabled:opacity-50 sm:text-xs"
          >
            调优组合
          </button>
          <button
            type="button"
            onClick={() => resetComposeForStock()}
            disabled={predicting}
            className="rounded-lg border border-white/10 px-2 py-1 text-[10px] text-slate-400 transition hover:bg-white/5 disabled:opacity-50 sm:text-xs"
          >
            重置
          </button>
        </div>
      </div>

      <div
        data-compose-scroll={bare ? undefined : true}
        className={cn(
          "space-y-1.5 pr-1",
          // bare：由外层 PredictPage 滚动，避免嵌套 overflow + overscroll-contain 吞掉滚轮
          bare ? "overflow-visible" : "overflow-y-auto overscroll-contain",
          !bare && (compact ? "max-h-[20rem]" : "max-h-[28rem] space-y-2"),
        )}
      >
        {compose.sources.map((src) => {
          const info = infoMap[src.id];
          const name = info?.name ?? src.id;
          const category = info?.category ?? "信号";
          const description = info?.description ?? `信号源 ${src.id}`;
          const available = info?.available ?? true;
          const backtestable = info?.backtestable ?? true;
          const weight = draftWeights[src.id] ?? src.weight;
          return (
            <div
              key={src.id}
              className={cn(
                "rounded-xl border transition",
                compact ? "px-2 py-2" : "px-3 py-2.5",
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
                  disabled={predicting || !available}
                  onClick={() => toggleSource(src.id)}
                  className={cn(
                    "mt-0.5 shrink-0 rounded-full border transition",
                    compact ? "h-4 w-7" : "h-5 w-9",
                    src.enabled
                      ? "border-cyan-400/50 bg-cyan-500/40"
                      : "border-white/10 bg-slate-700/50",
                    predicting && "opacity-50",
                  )}
                >
                  <span
                    className={cn(
                      "block rounded-full bg-white transition",
                      compact ? "h-3 w-3" : "h-4 w-4",
                      src.enabled
                        ? compact
                          ? "translate-x-3"
                          : "translate-x-4"
                        : "translate-x-0.5",
                    )}
                  />
                </button>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1">
                    <span
                      className={cn(
                        "font-medium text-slate-200",
                        compact ? "text-[11px] leading-tight sm:text-xs" : "text-sm",
                      )}
                    >
                      {name}
                    </span>
                    {!compact && (
                      <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] text-slate-500">
                        {category}
                      </span>
                    )}
                    {!backtestable && (
                      <span className="rounded bg-amber-500/10 px-1 py-0.5 text-[9px] text-amber-300/80 sm:text-[10px]">
                        仅实时
                      </span>
                    )}
                  </div>
                  {!compact && (
                    <p className="mt-0.5 text-[11px] leading-relaxed text-slate-500">{description}</p>
                  )}
                  {src.enabled && (
                    <WeightSlider
                      value={weight}
                      disabled={predicting}
                      compact={compact}
                      onDraft={(v) =>
                        setDraftWeights((prev) => ({
                          ...prev,
                          [src.id]: v,
                        }))
                      }
                      onCommit={(v) => setSourceWeight(src.id, v)}
                    />
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );

  if (bare) return <div className="pr-1">{body}</div>;

  return (
    <div
      className={cn(
        "rounded-2xl border border-white/5 bg-slate-900/50 backdrop-blur-sm",
        compact ? "p-2.5 sm:p-3" : "p-4",
      )}
    >
      {body}
    </div>
  );
}
