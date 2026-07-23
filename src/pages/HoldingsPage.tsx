import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { cn, formatPct, formatPrice } from "@/lib/utils";
import { floatingPnl, todayIsoDate } from "@/lib/pool";
import { useStockStore } from "@/stores/stockStore";
import type { Holding, Stock } from "@/types";

export function HoldingsPage() {
  const navigate = useNavigate();
  const holdings = useStockStore((s) => s.holdings);
  const stocks = useStockStore((s) => s.stocks);
  const hotStocks = useStockStore((s) => s.hotStocks);
  const selectStock = useStockStore((s) => s.selectStock);
  const upsertHolding = useStockStore((s) => s.upsertHolding);
  const removeHolding = useStockStore((s) => s.removeHolding);
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<Holding | null>(null);

  const quoteMap = useMemo(() => {
    const m = new Map<string, Stock>();
    for (const s of [...hotStocks, ...stocks]) m.set(s.code, s);
    return m;
  }, [stocks, hotStocks]);

  const rows = holdings.map((h) => {
    const q = quoteMap.get(h.code);
    const pnl = floatingPnl(h.cost, h.qty, q?.price);
    return { h, q, pnl };
  });

  const totalPnl = rows.reduce((acc, r) => acc + (r.pnl?.pnl ?? 0), 0);
  const hasPrice = rows.some((r) => r.pnl != null);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-white/5 px-3 py-3">
        <div className="flex items-start justify-between gap-2">
          <div>
            <h1 className="text-base font-semibold">持仓</h1>
            <p className="mt-1 text-[11px] text-slate-500">
              本地研究仓，不接券商 · 浮盈按会话行情估算
            </p>
          </div>
          <button
            type="button"
            onClick={() => {
              setEditing(null);
              setFormOpen(true);
            }}
            className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1.5 text-[11px] text-emerald-300"
          >
            登记持股
          </button>
        </div>
        <div className="mt-3 rounded-xl border border-white/5 bg-slate-900/50 px-3 py-2">
          <div className="text-[10px] text-slate-500">总浮盈</div>
          <div
            className={cn(
              "font-mono text-xl font-semibold tabular-nums",
              hasPrice
                ? totalPnl >= 0
                  ? "text-rose-400"
                  : "text-emerald-400"
                : "text-slate-500",
            )}
          >
            {hasPrice
              ? `${totalPnl >= 0 ? "+" : ""}${totalPnl.toFixed(0)}`
              : "—"}
          </div>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <div className="flex h-40 flex-col items-center justify-center gap-2 text-sm text-slate-500">
            暂无持仓
            <button
              type="button"
              className="text-cyan-400"
              onClick={() => setFormOpen(true)}
            >
              登记第一笔
            </button>
          </div>
        ) : (
          <ul className="divide-y divide-white/5">
            {rows.map(({ h, q, pnl }) => (
              <li key={h.code} className="px-3 py-3">
                <div className="flex items-start gap-2">
                  <button
                    type="button"
                    className="min-w-0 flex-1 text-left"
                    onClick={() => {
                      selectStock({
                        code: h.code,
                        name: h.name,
                        market: h.market,
                        sector: h.sector,
                        price: q?.price,
                        change_pct: q?.change_pct,
                      });
                      navigate(`/stock/${h.code}?tab=bs`);
                    }}
                  >
                    <div className="text-sm text-slate-100">{h.name}</div>
                    <div className="text-[11px] text-slate-500">
                      {h.code} · 成本 ¥{formatPrice(h.cost)} × {h.qty}
                    </div>
                    <div className="mt-1 flex flex-wrap gap-2 text-[11px]">
                      <span className="text-slate-400">
                        现价 {q?.price != null ? `¥${formatPrice(q.price)}` : "—"}
                      </span>
                      {pnl && (
                        <span
                          className={cn(
                            "font-mono",
                            pnl.pnl >= 0 ? "text-rose-400" : "text-emerald-400",
                          )}
                        >
                          {pnl.pnl >= 0 ? "+" : ""}
                          {pnl.pnl.toFixed(0)} ({formatPct(pnl.pct)})
                        </span>
                      )}
                    </div>
                  </button>
                  <button
                    type="button"
                    className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-cyan-300"
                    onClick={() => {
                      selectStock({
                        code: h.code,
                        name: h.name,
                        market: h.market,
                        sector: h.sector,
                      });
                      navigate(`/stock/${h.code}`);
                    }}
                  >
                    诊股
                  </button>
                  <button
                    type="button"
                    className="text-[10px] text-slate-400"
                    onClick={() => {
                      setEditing(h);
                      setFormOpen(true);
                    }}
                  >
                    改
                  </button>
                  <button
                    type="button"
                    className="text-[10px] text-rose-400"
                    onClick={() => removeHolding(h.code)}
                  >
                    删
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
        <div className="p-3 text-center text-[11px] text-slate-600">
          <Link to="/review" className="text-cyan-500">
            去复盘 →
          </Link>
        </div>
      </div>

      {formOpen && (
        <HoldingFormModal
          initial={editing}
          onClose={() => setFormOpen(false)}
          onSave={(h) => {
            upsertHolding(h);
            setFormOpen(false);
          }}
        />
      )}
    </div>
  );
}

function HoldingFormModal({
  initial,
  onClose,
  onSave,
}: {
  initial: Holding | null;
  onClose: () => void;
  onSave: (h: Holding) => void;
}) {
  const stocks = useStockStore((s) => s.stocks);
  const [code, setCode] = useState(initial?.code ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [market, setMarket] = useState(initial?.market ?? "SH");
  const [sector, setSector] = useState(initial?.sector ?? "");
  const [cost, setCost] = useState(String(initial?.cost ?? ""));
  const [qty, setQty] = useState(String(initial?.qty ?? ""));
  const [buyDate, setBuyDate] = useState(initial?.buyDate ?? todayIsoDate());
  const [note, setNote] = useState(initial?.note ?? "");

  const resolve = () => {
    const found = stocks.find((s) => s.code === code.trim());
    if (found) {
      setName(found.name);
      setMarket(found.market);
      setSector(found.sector);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 p-4 sm:items-center"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="max-h-[90dvh] w-full max-w-md overflow-y-auto rounded-2xl border border-white/10 bg-slate-900 p-4"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
      >
        <h2 className="text-base font-medium">{initial ? "编辑持仓" : "登记持股"}</h2>
        <div className="mt-3 space-y-2">
          <label className="block text-[11px] text-slate-400">
            代码
            <div className="mt-1 flex gap-2">
              <input
                className="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
                value={code}
                disabled={!!initial}
                onChange={(e) => setCode(e.target.value)}
                onBlur={resolve}
              />
              {!initial && (
                <button
                  type="button"
                  className="rounded-xl border border-white/10 px-2 text-xs text-cyan-300"
                  onClick={resolve}
                >
                  解析
                </button>
              )}
            </div>
          </label>
          <label className="block text-[11px] text-slate-400">
            名称
            <input
              className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <div className="grid grid-cols-2 gap-2">
            <label className="block text-[11px] text-slate-400">
              成本
              <input
                type="number"
                step="0.01"
                className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
                value={cost}
                onChange={(e) => setCost(e.target.value)}
              />
            </label>
            <label className="block text-[11px] text-slate-400">
              数量
              <input
                type="number"
                className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
                value={qty}
                onChange={(e) => setQty(e.target.value)}
              />
            </label>
          </div>
          <label className="block text-[11px] text-slate-400">
            买入日
            <input
              type="date"
              className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
              value={buyDate}
              onChange={(e) => setBuyDate(e.target.value)}
            />
          </label>
          <label className="block text-[11px] text-slate-400">
            备注
            <input
              className="mt-1 w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm"
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </label>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" className="text-sm text-slate-400" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="rounded-xl bg-emerald-500/20 px-3 py-2 text-sm text-emerald-300"
            onClick={() => {
              const c = Number(cost);
              const q = Number(qty);
              if (!code.trim() || !Number.isFinite(c) || !Number.isFinite(q) || q <= 0) return;
              onSave({
                code: code.trim(),
                name: name.trim() || code.trim(),
                market: market || "SH",
                sector: sector || "",
                cost: c,
                qty: q,
                buyDate,
                note,
              });
            }}
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
