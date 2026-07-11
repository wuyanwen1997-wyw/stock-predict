import { motion } from "framer-motion";
import { useMemo, useState } from "react";
import { cn, formatPct, formatPrice, marketLabel } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import type { Stock } from "@/types";

function StockRow({
  stock,
  active,
  starred,
  index,
  onSelect,
  onToggleWatchlist,
}: {
  stock: Stock;
  active: boolean;
  starred: boolean;
  index: number;
  onSelect: () => void;
  onToggleWatchlist: () => void;
}) {
  const changePct = stock.change_pct;
  const up = changePct != null && changePct > 0;
  const down = changePct != null && changePct < 0;

  return (
    <motion.div
      initial={{ opacity: 0, x: -12 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: index * 0.03 }}
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onSelect();
      }}
      className={cn(
        "group relative flex cursor-pointer items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all duration-200",
        active
          ? "border-emerald-500/40 bg-emerald-500/10 shadow-lg shadow-emerald-500/10"
          : "border-white/5 bg-slate-800/30 hover:border-white/10 hover:bg-slate-800/60",
      )}
    >
      <div
        className={cn(
          "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-xs font-bold",
          active
            ? "bg-emerald-500/20 text-emerald-300"
            : "bg-slate-700/50 text-slate-400",
        )}
      >
        {marketLabel(stock.market)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-sm font-medium text-slate-200">
            {stock.name}
          </span>
          {stock.is_hot && (
            <span className="shrink-0 rounded bg-orange-500/15 px-1.5 py-0.5 text-[10px] font-medium text-orange-300">
              热
            </span>
          )}
        </div>
        <div className="truncate text-xs text-slate-500">
          {stock.code} · {stock.sector}
        </div>
      </div>

      {stock.price != null && (
        <div className="shrink-0 text-right">
          <div className="font-mono text-sm tabular-nums text-slate-200">
            ¥{formatPrice(stock.price)}
          </div>
          {changePct != null && (
            <div
              className={cn(
                "font-mono text-xs tabular-nums",
                up && "text-rose-400",
                down && "text-emerald-400",
                !up && !down && "text-slate-500",
              )}
            >
              {formatPct(changePct)}
            </div>
          )}
        </div>
      )}

      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onToggleWatchlist();
        }}
        className={cn(
          "shrink-0 text-sm transition-colors",
          starred ? "text-amber-400" : "text-slate-600 hover:text-amber-400/70",
        )}
        aria-label={starred ? "取消自选" : "加入自选"}
      >
        {starred ? "★" : "☆"}
      </button>
    </motion.div>
  );
}

export function StockSelector() {
  const stocks = useStockStore((s) => s.stocks);
  const hotStocks = useStockStore((s) => s.hotStocks);
  const searchResults = useStockStore((s) => s.searchResults);
  const searchQuery = useStockStore((s) => s.searchQuery);
  const searching = useStockStore((s) => s.searching);
  const selectedStock = useStockStore((s) => s.selectedStock);
  const watchlist = useStockStore((s) => s.watchlist);
  const selectStock = useStockStore((s) => s.selectStock);
  const toggleWatchlist = useStockStore((s) => s.toggleWatchlist);
  const setSearchQuery = useStockStore((s) => s.setSearchQuery);
  const runSearch = useStockStore((s) => s.runSearch);
  const clearSearch = useStockStore((s) => s.clearSearch);

  const [showAll, setShowAll] = useState(false);

  const displayStocks = useMemo(() => {
    if (searchQuery.trim()) return searchResults;
    if (showAll) return stocks;
    if (hotStocks.length > 0) return hotStocks;
    return stocks.slice(0, 8);
  }, [searchQuery, searchResults, showAll, stocks, hotStocks]);

  const sectionTitle = searchQuery.trim()
    ? "搜索结果"
    : showAll
      ? "全部股票"
      : hotStocks.length > 0
        ? "热门股票"
        : "推荐股票";

  return (
    <div className="space-y-4">
      <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-4 backdrop-blur-sm">
        <div className="mb-3">
          <h2 className="text-sm font-medium text-slate-300">搜索股票</h2>
          <p className="mt-1 text-xs text-slate-500">输入代码或名称，搜索全市场 A 股</p>
        </div>

        <div className="flex gap-2">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void runSearch();
            }}
            placeholder="如 600519、茅台"
            className="flex-1 rounded-xl border border-white/10 bg-slate-800/60 px-3 py-2 text-sm text-slate-200 outline-none transition placeholder:text-slate-600 focus:border-emerald-500/40"
          />
          <button
            type="button"
            onClick={() => void runSearch()}
            disabled={searching || !searchQuery.trim()}
            className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300 transition hover:bg-emerald-500/20 disabled:opacity-50"
          >
            {searching ? "..." : "搜"}
          </button>
          {searchQuery && (
            <button
              type="button"
              onClick={clearSearch}
              className="rounded-xl border border-white/10 px-3 py-2 text-sm text-slate-400 transition hover:bg-white/5"
            >
              清
            </button>
          )}
        </div>
      </div>

      <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-4 backdrop-blur-sm">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-medium text-slate-300">{sectionTitle}</h2>
            {!searchQuery.trim() && hotStocks.length > 0 && !showAll && (
              <p className="mt-0.5 text-xs text-slate-500">东方财富人气榜 · 实时行情</p>
            )}
          </div>
          <div className="flex items-center gap-2">
            {!searchQuery.trim() && (
              <button
                type="button"
                onClick={() => setShowAll((v) => !v)}
                className="text-xs text-slate-500 transition hover:text-slate-300"
              >
                {showAll ? "看热门" : `全部 ${stocks.length}`}
              </button>
            )}
            <span className="text-xs text-slate-500">{displayStocks.length} 只</span>
          </div>
        </div>

        <div className="grid max-h-80 gap-2 overflow-y-auto pr-1 sm:grid-cols-1">
          {displayStocks.length === 0 ? (
            <div className="flex h-24 items-center justify-center text-sm text-slate-500">
              {searching ? "搜索中..." : searchQuery.trim() ? "未找到匹配股票" : "暂无数据"}
            </div>
          ) : (
            displayStocks.map((stock, i) => (
              <StockRow
                key={`${stock.market}-${stock.code}`}
                stock={stock}
                active={selectedStock?.code === stock.code}
                starred={watchlist.includes(stock.code)}
                index={i}
                onSelect={() => selectStock(stock)}
                onToggleWatchlist={() => toggleWatchlist(stock.code)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
