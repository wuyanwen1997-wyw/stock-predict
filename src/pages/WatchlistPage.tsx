import { motion } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { cn, formatPct, formatPrice, marketLabel } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

export function WatchlistPage() {
  const navigate = useNavigate();
  const watchlist = useStockStore((s) => s.watchlist);
  const selectStock = useStockStore((s) => s.selectStock);
  const toggleWatchlist = useStockStore((s) => s.toggleWatchlist);

  const watched = watchlist;

  const goPredict = (stock: (typeof watched)[number]) => {
    selectStock(stock);
    navigate("/predict");
  };

  return (
    <div className="h-full min-h-0 overflow-y-auto p-4 sm:p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -12 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-6"
      >
        <h1 className="text-xl font-semibold text-slate-100 sm:text-2xl">自选股</h1>
        <p className="mt-1.5 text-sm text-slate-400">点击卡片进入预测，点 ★ 可移除。</p>
      </motion.header>

      {watched.length === 0 ? (
        <div className="flex h-48 items-center justify-center rounded-2xl border border-dashed border-white/10 text-slate-500">
          暂无自选股，在首页点击 ☆ 添加
        </div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {watched.map((stock, i) => (
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
              <div className="mt-3 text-xs text-emerald-400/80">点击进入预测 →</div>
            </motion.div>
          ))}
        </div>
      )}
    </div>
  );
}
