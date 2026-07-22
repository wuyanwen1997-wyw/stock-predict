import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { cn, formatPct, formatPrice, marketLabel } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import {
  conditionSummary,
  useMonitorStore,
} from "@/stores/monitorStore";
import type { AlertCondition, MonitorRule, Stock } from "@/types";

export function WatchlistPage() {
  const navigate = useNavigate();
  const watchlist = useStockStore((s) => s.watchlist);
  const selectStock = useStockStore((s) => s.selectStock);
  const toggleWatchlist = useStockStore((s) => s.toggleWatchlist);

  const rules = useMonitorStore((s) => s.rules);
  const alerts = useMonitorStore((s) => s.alerts);
  const running = useMonitorStore((s) => s.running);
  const starting = useMonitorStore((s) => s.starting);
  const error = useMonitorStore((s) => s.error);
  const setMonitoring = useMonitorStore((s) => s.setMonitoring);
  const ensureListeners = useMonitorStore((s) => s.ensureListeners);
  const clearAlerts = useMonitorStore((s) => s.clearAlerts);
  const removeRule = useMonitorStore((s) => s.removeRule);
  const upsertRule = useMonitorStore((s) => s.upsertRule);
  const syncAndMaybeRestart = useMonitorStore((s) => s.syncAndMaybeRestart);

  const [editStock, setEditStock] = useState<Stock | null>(null);
  const [showAlerts, setShowAlerts] = useState(false);

  const watchCodes = useMemo(
    () => watchlist.map((s) => s.code).join(","),
    [watchlist],
  );

  useEffect(() => {
    void ensureListeners();
  }, [ensureListeners]);

  useEffect(() => {
    if (running) {
      void syncAndMaybeRestart();
    }
  }, [watchCodes, rules, running, syncAndMaybeRestart]);

  const ruleCountByCode = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of rules) {
      if (!r.enabled) continue;
      m.set(r.code, (m.get(r.code) ?? 0) + 1);
    }
    return m;
  }, [rules]);

  const goPredict = (stock: Stock) => {
    selectStock(stock);
    navigate("/predict");
  };

  return (
    <div className="h-full min-h-0 overflow-y-auto p-4 sm:p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -12 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-4"
      >
        <div className="flex items-start justify-between gap-3">
          <div>
            <h1 className="text-xl font-semibold text-slate-100 sm:text-2xl">自选股</h1>
            <p className="mt-1.5 text-sm text-slate-400">
              盯盘开启后锁屏也可监控；触达将弹出系统通知。
            </p>
          </div>
          <button
            type="button"
            disabled={starting || watchlist.length === 0}
            onClick={() => void setMonitoring(!running)}
            className={cn(
              "shrink-0 rounded-xl px-3 py-2 text-sm font-medium transition",
              running
                ? "bg-emerald-500/20 text-emerald-300 ring-1 ring-emerald-500/40"
                : "bg-slate-800 text-slate-300 ring-1 ring-white/10",
              (starting || watchlist.length === 0) && "opacity-50",
            )}
          >
            {starting ? "…" : running ? "盯盘中" : "开启盯盘"}
          </button>
        </div>
        {running && (
          <p className="mt-2 text-xs text-amber-400/90">
            Android 将显示常驻「盯盘中」通知以保持锁屏监控；强制停止应用后需重新开启。
          </p>
        )}
        {error && <p className="mt-2 text-xs text-rose-400">{error}</p>}
      </motion.header>

      {alerts.length > 0 && (
        <div className="mb-4 rounded-xl border border-amber-500/20 bg-amber-500/5 p-3">
          <div className="flex items-center justify-between gap-2">
            <button
              type="button"
              className="text-sm text-amber-200"
              onClick={() => setShowAlerts((v) => !v)}
            >
              最近预警 {alerts.length} 条 {showAlerts ? "▴" : "▾"}
            </button>
            <button
              type="button"
              className="text-xs text-slate-500 hover:text-slate-300"
              onClick={() => clearAlerts()}
            >
              清空
            </button>
          </div>
          {showAlerts && (
            <ul className="mt-2 max-h-40 space-y-1.5 overflow-y-auto text-xs text-slate-300">
              {alerts.slice(0, 20).map((a) => (
                <li key={a.id} className="rounded-lg bg-slate-900/60 px-2 py-1.5">
                  <span className="font-medium text-slate-200">{a.name}</span>
                  <span className="ml-2 text-slate-500">
                    {new Date(a.fired_at).toLocaleString()}
                  </span>
                  <div className="text-slate-400">{a.message}</div>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {watchlist.length === 0 ? (
        <div className="flex h-48 items-center justify-center rounded-2xl border border-dashed border-white/10 text-slate-500">
          暂无自选股，在首页点击 ☆ 添加
        </div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {watchlist.map((stock, i) => (
            <motion.div
              key={stock.code}
              role="button"
              tabIndex={0}
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: i * 0.05 }}
              onClick={() => goPredict(stock)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  goPredict(stock);
                }
              }}
              className="cursor-pointer rounded-2xl border border-white/5 bg-slate-900/50 p-4 backdrop-blur-sm transition active:border-emerald-500/30 active:bg-slate-900/80"
            >
              <div className="flex items-start gap-3">
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-slate-800 text-sm font-bold text-slate-400">
                  {stock.sector === "ETF" ? "ETF" : marketLabel(stock.market)}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-slate-200">{stock.name}</div>
                  <div className="text-xs text-slate-500">
                    {stock.code} · {stock.sector}
                    {(ruleCountByCode.get(stock.code) ?? 0) > 0 && (
                      <span className="ml-2 text-cyan-400/90">
                        {ruleCountByCode.get(stock.code)} 条预警
                      </span>
                    )}
                  </div>
                  {stock.price != null && (
                    <div className="mt-1 font-mono text-sm tabular-nums text-slate-300">
                      ¥{formatPrice(stock.price)}
                      {stock.change_pct != null && (
                        <span
                          className={cn(
                            "ml-2 text-xs",
                            stock.change_pct > 0 && "text-rose-400",
                            stock.change_pct < 0 && "text-emerald-400",
                          )}
                        >
                          {formatPct(stock.change_pct)}
                        </span>
                      )}
                    </div>
                  )}
                </div>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleWatchlist(stock);
                  }}
                  className="shrink-0 px-1 text-lg text-amber-400"
                  aria-label="取消自选"
                >
                  ★
                </button>
              </div>
              <div className="mt-3 flex items-center justify-between gap-2 text-xs">
                <span className="text-emerald-400/80">点击进入预测 →</span>
                <button
                  type="button"
                  className="rounded-lg bg-slate-800 px-2 py-1 text-cyan-300 ring-1 ring-white/10"
                  onClick={(e) => {
                    e.stopPropagation();
                    setEditStock(stock);
                  }}
                >
                  设预警
                </button>
              </div>
            </motion.div>
          ))}
        </div>
      )}

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

  const buildCondition = (): AlertCondition | null => {
    const n = Number(value);
    if (!Number.isFinite(n)) return null;
    return { kind, value: n };
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 p-4 sm:items-center"
      onClick={onClose}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
      role="presentation"
    >
      <div
        className="w-full max-w-md rounded-2xl border border-white/10 bg-slate-900 p-4 shadow-xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal
      >
        <h2 className="text-lg font-medium text-slate-100">
          {stock.name} · 预警
        </h2>
        <p className="mt-1 text-xs text-slate-500">{stock.code}</p>

        {rules.length > 0 && (
          <ul className="mt-3 space-y-2">
            {rules.map((r) => (
              <li
                key={r.id}
                className="flex items-center justify-between gap-2 rounded-xl bg-slate-800/80 px-3 py-2 text-sm"
              >
                <span className="text-slate-200">{conditionSummary(r.condition)}</span>
                <button
                  type="button"
                  className="text-xs text-rose-400"
                  onClick={() => onRemove(r.id)}
                >
                  删除
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className="mt-4 space-y-3">
          <label className="block text-xs text-slate-400">
            条件类型
            <select
              className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm text-slate-200"
              value={kind}
              onChange={(e) => setKind(e.target.value as AlertCondition["kind"])}
            >
              <option value="change_pct_above">涨跌幅 ≥</option>
              <option value="change_pct_below">涨跌幅 ≤</option>
              <option value="price_above">价格 ≥</option>
              <option value="price_below">价格 ≤</option>
            </select>
          </label>
          <label className="block text-xs text-slate-400">
            阈值{kind.includes("pct") ? "（%）" : "（元）"}
            <input
              type="number"
              step="0.01"
              className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm text-slate-200"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </label>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            className="rounded-xl px-3 py-2 text-sm text-slate-400"
            onClick={onClose}
          >
            取消
          </button>
          <button
            type="button"
            className="rounded-xl bg-cyan-500/20 px-3 py-2 text-sm text-cyan-300 ring-1 ring-cyan-500/40"
            onClick={() => {
              const condition = buildCondition();
              if (!condition) return;
              onSave({
                code: stock.code,
                name: stock.name,
                enabled: true,
                condition,
                cooldown_sec: 300,
                max_per_day: 5,
              });
            }}
          >
            添加预警
          </button>
        </div>
      </div>
    </div>
  );
}
