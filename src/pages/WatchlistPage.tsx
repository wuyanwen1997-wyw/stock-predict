import { motion } from "framer-motion";
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { cn, marketLabel } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

export function WatchlistPage() {
  const navigate = useNavigate();
  const stocks = useStockStore((s) => s.stocks);
  const watchlist = useStockStore((s) => s.watchlist);
  const selectStock = useStockStore((s) => s.selectStock);
  const toggleWatchlist = useStockStore((s) => s.toggleWatchlist);

  const watched = useMemo(
    () => stocks.filter((s) => watchlist.includes(s.code)),
    [stocks, watchlist],
  );

  return (
    <div className="p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -12 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-8"
      >
        <h1 className="text-2xl font-semibold text-slate-100">自选股</h1>
        <p className="mt-2 text-sm text-slate-400">
          管理你关注的股票，点击可快速跳转预测。
        </p>
      </motion.header>

      {watched.length === 0 ? (
        <div className="flex h-48 items-center justify-center rounded-2xl border border-dashed border-white/10 text-slate-500">
          暂无自选股，在预测页点击 ☆ 添加
        </div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {watched.map((stock, i) => (
            <motion.div
              key={stock.code}
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: i * 0.05 }}
              className="group rounded-2xl border border-white/5 bg-slate-900/50 p-4 backdrop-blur-sm transition hover:border-emerald-500/20"
            >
              <div className="flex items-start gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-slate-800 text-sm font-bold text-slate-400">
                  {marketLabel(stock.market)}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-slate-200">{stock.name}</div>
                  <div className="text-xs text-slate-500">
                    {stock.code} · {stock.sector}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => toggleWatchlist(stock.code)}
                  className="text-amber-400 transition hover:text-amber-300"
                >
                  ★
                </button>
              </div>

              <button
                type="button"
                onClick={() => {
                  selectStock(stock);
                  navigate("/");
                }}
                className={cn(
                  "mt-4 w-full rounded-xl border border-emerald-500/20 bg-emerald-500/10 py-2 text-sm text-emerald-300",
                  "opacity-0 transition group-hover:opacity-100 hover:bg-emerald-500/20",
                )}
              >
                查看预测 →
              </button>
            </motion.div>
          ))}
        </div>
      )}
    </div>
  );
}
