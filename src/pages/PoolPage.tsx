import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { cn, formatPct, formatPrice, marketLabel } from "@/lib/utils";
import {
  GROUP_BUY,
  GROUP_HOLDINGS_MIRROR,
  GROUP_REMOVED,
  newGroupId,
  stockFromPoolItem,
} from "@/lib/pool";
import { useStockStore } from "@/stores/stockStore";
import { conditionSummary, useMonitorStore } from "@/stores/monitorStore";
import { AddToPoolModal } from "@/components/AddToPoolModal";
import type { AlertCondition, MonitorRule, Stock } from "@/types";

export function PoolPage() {
  const navigate = useNavigate();
  const poolGroups = useStockStore((s) => s.poolGroups);
  const poolItems = useStockStore((s) => s.poolItems);
  const selectStock = useStockStore((s) => s.selectStock);
  const removeFromPool = useStockStore((s) => s.removeFromPool);
  const movePoolItem = useStockStore((s) => s.movePoolItem);
  const upsertPoolGroup = useStockStore((s) => s.upsertPoolGroup);
  const removePoolGroup = useStockStore((s) => s.removePoolGroup);
  const stocks = useStockStore((s) => s.stocks);

  const rules = useMonitorStore((s) => s.rules);
  const alerts = useMonitorStore((s) => s.alerts);
  const running = useMonitorStore((s) => s.running);
  const starting = useMonitorStore((s) => s.starting);
  const setMonitoring = useMonitorStore((s) => s.setMonitoring);
  const ensureListeners = useMonitorStore((s) => s.ensureListeners);
  const syncAndMaybeRestart = useMonitorStore((s) => s.syncAndMaybeRestart);
  const upsertRule = useMonitorStore((s) => s.upsertRule);
  const removeRule = useMonitorStore((s) => s.removeRule);
  const clearAlerts = useMonitorStore((s) => s.clearAlerts);

  const visibleGroups = useMemo(
    () =>
      [...poolGroups]
        .filter((g) => g.id !== GROUP_HOLDINGS_MIRROR || poolItems.some((i) => i.groupId === g.id))
        .sort((a, b) => a.sortOrder - b.sortOrder),
    [poolGroups, poolItems],
  );

  const [activeGroup, setActiveGroup] = useState("g_watch");
  const [buyFilter, setBuyFilter] = useState(false);
  const [editStock, setEditStock] = useState<Stock | null>(null);
  const [addStock, setAddStock] = useState<Stock | null>(null);
  const [newGroupName, setNewGroupName] = useState("");
  const [showAlerts, setShowAlerts] = useState(false);

  useEffect(() => {
    void ensureListeners();
  }, [ensureListeners]);

  const groupCodes = useMemo(
    () =>
      poolItems
        .filter((i) => i.groupId === activeGroup)
        .map((i) => i.code)
        .join(","),
    [poolItems, activeGroup],
  );

  useEffect(() => {
    if (running) void syncAndMaybeRestart();
  }, [groupCodes, rules, running, syncAndMaybeRestart]);

  const items = useMemo(() => {
    let list = poolItems.filter((i) => i.groupId === activeGroup);
    if (buyFilter) {
      // 「今日买点」启发式：优先看待买组；否则展示全部（真实 B/S 需进个股）
      if (activeGroup !== GROUP_BUY) {
        list = poolItems.filter((i) => i.groupId === GROUP_BUY);
      }
    }
    return list.sort((a, b) => a.sortOrder - b.sortOrder);
  }, [poolItems, activeGroup, buyFilter]);

  const quoteMap = useMemo(() => {
    const m = new Map(stocks.map((s) => [s.code, s]));
    return m;
  }, [stocks]);

  const goStock = (code: string) => {
    const item = poolItems.find((i) => i.code === code);
    const q = quoteMap.get(code);
    const stock = q ?? (item ? stockFromPoolItem(item) : null);
    if (!stock) return;
    selectStock(stock);
    navigate(`/stock/${code}`);
  };

  const poolStockCount = new Set(
    poolItems.filter((i) => i.groupId !== GROUP_HOLDINGS_MIRROR).map((i) => i.code),
  ).size;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 space-y-2 border-b border-white/5 px-3 py-3">
        <div className="flex items-center gap-2">
          <h1 className="text-base font-semibold">股票池</h1>
          <button
            type="button"
            disabled={starting || poolStockCount === 0}
            onClick={() => void setMonitoring(!running)}
            className={cn(
              "rounded-lg px-2 py-1 text-[11px]",
              running
                ? "bg-emerald-500/20 text-emerald-300"
                : "bg-slate-800 text-slate-300",
            )}
          >
            {starting ? "…" : running ? "盯盘中" : "盯盘"}
          </button>
          <button
            type="button"
            onClick={() => {
              const codes = items.slice(0, 4).map((i) => i.code).join(",");
              navigate(codes ? `/compare?codes=${codes}` : "/compare");
            }}
            className="rounded-lg border border-white/10 px-2 py-1 text-[11px] text-slate-300"
          >
            同步
          </button>
          <button
            type="button"
            onClick={() => setBuyFilter((v) => !v)}
            className={cn(
              "rounded-lg border px-2 py-1 text-[11px]",
              buyFilter
                ? "border-amber-500/40 text-amber-300"
                : "border-white/10 text-slate-400",
            )}
          >
            今日买点
          </button>
          <button
            type="button"
            onClick={() => {
              const seed = stocks[0];
              if (seed) setAddStock(seed);
            }}
            className="ml-auto rounded-lg border border-emerald-500/30 px-2 py-1 text-[11px] text-emerald-300"
          >
            +
          </button>
        </div>

        <div className="-mx-1 flex gap-1.5 overflow-x-auto px-1 [scrollbar-width:none]">
          {visibleGroups.map((g) => (
            <button
              key={g.id}
              type="button"
              onClick={() => {
                setActiveGroup(g.id);
                setBuyFilter(false);
              }}
              className={cn(
                "shrink-0 rounded-full px-3 py-1 text-xs",
                activeGroup === g.id && !buyFilter
                  ? "bg-emerald-500/20 text-emerald-300"
                  : "bg-white/5 text-slate-400",
              )}
            >
              {g.name}
              <span className="ml-1 text-[10px] opacity-60">
                {poolItems.filter((i) => i.groupId === g.id).length}
              </span>
            </button>
          ))}
        </div>

        <div className="flex gap-2">
          <input
            value={newGroupName}
            onChange={(e) => setNewGroupName(e.target.value)}
            placeholder="新建分组名"
            className="min-w-0 flex-1 rounded-lg border border-white/10 bg-slate-900/60 px-2 py-1 text-xs"
          />
          <button
            type="button"
            className="rounded-lg border border-white/10 px-2 py-1 text-[11px] text-cyan-300"
            onClick={() => {
              const name = newGroupName.trim();
              if (!name) return;
              upsertPoolGroup({
                id: newGroupId(),
                name,
                sortOrder: poolGroups.length,
                kind: "user",
              });
              setNewGroupName("");
            }}
          >
            建组
          </button>
          {activeGroup.startsWith("g_") &&
            !["g_watch", "g_buy", "g_observe", "g_removed", "g_holdings_mirror"].includes(
              activeGroup,
            ) && (
              <button
                type="button"
                className="rounded-lg px-2 py-1 text-[11px] text-rose-400"
                onClick={() => removePoolGroup(activeGroup)}
              >
                删组
              </button>
            )}
        </div>

        {alerts.length > 0 && (
          <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-2 text-xs">
            <button type="button" onClick={() => setShowAlerts((v) => !v)}>
              预警 {alerts.length} {showAlerts ? "▴" : "▾"}
            </button>
            <button type="button" className="ml-3 text-slate-500" onClick={() => clearAlerts()}>
              清空
            </button>
            {showAlerts && (
              <ul className="mt-1 max-h-28 overflow-y-auto text-slate-400">
                {alerts.slice(0, 10).map((a) => (
                  <li key={a.id}>
                    {a.name}: {a.message}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {items.length === 0 ? (
          <div className="flex h-40 items-center justify-center text-sm text-slate-500">
            本组暂无标的 · 从选股或行情入池
          </div>
        ) : (
          <ul className="divide-y divide-white/5">
            {items.map((item) => {
              const q = quoteMap.get(item.code);
              const changePct = q?.change_pct;
              const stock = q ?? stockFromPoolItem(item);
              return (
                <li key={`${item.groupId}-${item.code}`} className="flex items-center gap-2 px-3 py-3">
                  <button
                    type="button"
                    className="min-w-0 flex-1 text-left"
                    onClick={() => goStock(item.code)}
                  >
                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-slate-500">
                        {marketLabel(item.market)}
                      </span>
                      <span className="truncate text-sm text-slate-100">{item.name}</span>
                      <span className="font-mono text-[11px] text-slate-500">{item.code}</span>
                    </div>
                    <div className="mt-0.5 flex gap-2 text-[11px] text-slate-500">
                      {q?.price != null && <span>¥{formatPrice(q.price)}</span>}
                      {changePct != null && (
                        <span
                          className={cn(
                            changePct > 0 && "text-rose-400",
                            changePct < 0 && "text-emerald-400",
                          )}
                        >
                          {formatPct(changePct)}
                        </span>
                      )}
                    </div>
                  </button>
                  <button
                    type="button"
                    className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-cyan-300"
                    onClick={() => goStock(item.code)}
                  >
                    诊
                  </button>
                  <button
                    type="button"
                    className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-slate-400"
                    onClick={() => setEditStock(stock)}
                  >
                    盯
                  </button>
                  {activeGroup !== GROUP_REMOVED && (
                    <button
                      type="button"
                      className="text-[10px] text-slate-500"
                      onClick={() => movePoolItem(item.code, item.groupId, GROUP_REMOVED)}
                    >
                      剔
                    </button>
                  )}
                  <button
                    type="button"
                    className="text-[10px] text-rose-400/80"
                    onClick={() => removeFromPool(item.code, item.groupId)}
                  >
                    ×
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {editStock && (
        <RuleEditorModal
          stock={editStock}
          rules={rules.filter((r) => r.code === editStock.code)}
          onClose={() => setEditStock(null)}
          onSave={(rule) => {
            upsertRule(rule);
            setEditStock(null);
          }}
          onRemove={(id) => removeRule(id)}
        />
      )}
      {addStock && (
        <AddToPoolModal
          stock={addStock}
          defaultGroupId={activeGroup === GROUP_HOLDINGS_MIRROR ? "g_watch" : activeGroup}
          onClose={() => setAddStock(null)}
        />
      )}
    </div>
  );
}

function RuleEditorModal({
  stock,
  rules,
  onClose,
  onSave,
  onRemove,
}: {
  stock: Stock;
  rules: MonitorRule[];
  onClose: () => void;
  onSave: (rule: Omit<MonitorRule, "id" | "created_at"> & { id?: string }) => void;
  onRemove: (id: string) => void;
}) {
  const [kind, setKind] = useState<AlertCondition["kind"]>("change_pct_above");
  const [value, setValue] = useState("3");

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 p-4 sm:items-center"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="w-full max-w-md rounded-2xl border border-white/10 bg-slate-900 p-4"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
      >
        <h2 className="text-base text-slate-100">{stock.name} · 预警</h2>
        {rules.map((r) => (
          <div key={r.id} className="mt-2 flex justify-between text-sm">
            <span>{conditionSummary(r.condition)}</span>
            <button type="button" className="text-rose-400" onClick={() => onRemove(r.id)}>
              删
            </button>
          </div>
        ))}
        <select
          className="mt-3 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
          value={kind}
          onChange={(e) => setKind(e.target.value as AlertCondition["kind"])}
        >
          <option value="change_pct_above">涨跌幅 ≥</option>
          <option value="change_pct_below">涨跌幅 ≤</option>
          <option value="price_above">价格 ≥</option>
          <option value="price_below">价格 ≤</option>
        </select>
        <input
          type="number"
          className="mt-2 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <div className="mt-3 flex justify-end gap-2">
          <button type="button" className="text-sm text-slate-400" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="rounded-xl bg-cyan-500/20 px-3 py-2 text-sm text-cyan-300"
            onClick={() => {
              const n = Number(value);
              if (!Number.isFinite(n)) return;
              onSave({
                code: stock.code,
                name: stock.name,
                enabled: true,
                condition: { kind, value: n },
                cooldown_sec: 300,
                max_per_day: 5,
              });
            }}
          >
            添加
          </button>
        </div>
      </div>
    </div>
  );
}
