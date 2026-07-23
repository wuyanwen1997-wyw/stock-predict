import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { cn, formatPct, formatPrice, marketLabel } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import type { Stock } from "@/types";
import { AddToPoolModal } from "@/components/AddToPoolModal";

/** Preferred board order (Tonghuashun-like). */
const BOARD_ORDER = [
  "热门",
  "ETF",
  "白酒",
  "银行",
  "证券",
  "保险",
  "新能源",
  "光伏",
  "半导体",
  "AI",
  "医药",
  "消费",
  "家电",
  "电力",
  "有色",
  "安防",
  "面板",
] as const;

export function HomePage() {
  const loading = useStockStore((s) => s.loading);
  const error = useStockStore((s) => s.error);
  const hotStocks = useStockStore((s) => s.hotStocks);
  const stocks = useStockStore((s) => s.stocks);
  const searchResults = useStockStore((s) => s.searchResults);
  const searchQuery = useStockStore((s) => s.searchQuery);
  const searching = useStockStore((s) => s.searching);
  const watchlist = useStockStore((s) => s.watchlist);
  const selectStock = useStockStore((s) => s.selectStock);
  const setSearchQuery = useStockStore((s) => s.setSearchQuery);
  const runSearch = useStockStore((s) => s.runSearch);
  const clearSearch = useStockStore((s) => s.clearSearch);
  const init = useStockStore((s) => s.init);
  const navigate = useNavigate();

  const [board, setBoard] = useState<string>("热门");
  const [poolStock, setPoolStock] = useState<Stock | null>(null);

  const boards = useMemo(() => {
    const present = new Set(stocks.map((s) => s.sector).filter(Boolean));
    const ordered: string[] = BOARD_ORDER.filter(
      (b) => b === "热门" || present.has(b),
    );
    for (const s of present) {
      if (!ordered.includes(s)) ordered.push(s);
    }
    return ordered;
  }, [stocks]);

  const list = useMemo(() => {
    if (searchQuery.trim()) return searchResults;
    if (board === "热门") {
      if (hotStocks.length > 0) return hotStocks;
      return stocks.filter((s) => s.is_hot).slice(0, 20);
    }
    if (board === "ETF") return stocks.filter((s) => s.sector === "ETF");
    return stocks.filter((s) => s.sector === board);
  }, [searchQuery, searchResults, board, hotStocks, stocks]);

  const onPick = (stock: Stock) => {
    selectStock(stock);
    navigate(`/stock/${stock.code}`);
  };

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="animate-shimmer h-32 w-64 rounded-2xl" />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 space-y-2 border-b border-white/5 bg-slate-950/80 px-3 py-3 backdrop-blur-xl">
        <div className="flex items-center gap-2">
          <h1 className="text-base font-semibold text-slate-100">行情</h1>
          <button
            type="button"
            onClick={() => void init()}
            className="ml-auto rounded-lg border border-white/10 px-2 py-1 text-[11px] text-slate-400"
          >
            刷新
          </button>
        </div>

        <div className="flex gap-1.5 overflow-x-auto [scrollbar-width:none]">
          <LinkChip to="/screen" label="盘前选股" />
          <LinkChip to="/pool" label="盘中看池" />
          <LinkChip to="/review" label="盘后复盘" />
          <LinkChip to="/compare" label="多股对比" />
        </div>

        <div className="flex gap-2">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void runSearch();
            }}
            placeholder="代码 / 名称 / ETF，输入即搜"
            className="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-800/60 px-3 py-2 text-sm text-slate-200 outline-none placeholder:text-slate-600 focus:border-emerald-500/40"
          />
          <button
            type="button"
            onClick={() => void runSearch()}
            disabled={searching || !searchQuery.trim()}
            className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300 disabled:opacity-50"
          >
            {searching ? "..." : "搜"}
          </button>
          {searchQuery && (
            <button
              type="button"
              onClick={clearSearch}
              className="rounded-xl border border-white/10 px-3 py-2 text-sm text-slate-400"
            >
              清
            </button>
          )}
        </div>

        {!searchQuery.trim() && (
          <div className="-mx-1 flex gap-1.5 overflow-x-auto px-1 pb-0.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
            {boards.map((b) => (
              <button
                key={b}
                type="button"
                onClick={() => setBoard(b)}
                className={cn(
                  "shrink-0 rounded-full px-3 py-1 text-xs font-medium transition",
                  board === b
                    ? b === "ETF"
                      ? "bg-violet-500/25 text-violet-200"
                      : "bg-emerald-500/20 text-emerald-300"
                    : "bg-white/5 text-slate-400",
                )}
              >
                {b}
              </button>
            ))}
          </div>
        )}

        {error && (
          <p className="text-[11px] text-rose-300">{error}</p>
        )}
        {!searchQuery.trim() && board === "热门" && hotStocks.length === 0 && (
          <p className="text-[11px] text-slate-500">
            {error ? "人气榜暂不可用，可切换板块或搜索" : "人气榜暂无数据，可切换板块或搜索"}
          </p>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
        <div className="px-3 py-2 text-[11px] text-slate-500">
          {searchQuery.trim() ? "搜索结果" : board}
          <span className="ml-1 text-slate-600">· {list.length} 只</span>
        </div>

        {list.length === 0 ? (
          <div className="flex h-40 items-center justify-center text-sm text-slate-500">
            {searching ? "搜索中..." : searchQuery.trim() ? "未找到匹配" : "该板块暂无标的"}
          </div>
        ) : (
          <ul className="divide-y divide-white/5">
            {list.map((stock) => {
              const changePct = stock.change_pct;
              const up = changePct != null && changePct > 0;
              const down = changePct != null && changePct < 0;
              const starred = watchlist.some((s) => s.code === stock.code);
              return (
                <li key={`${stock.market}-${stock.code}`}>
                  <div className="flex w-full items-center gap-3 px-3 py-3 active:bg-white/5">
                    <button
                      type="button"
                      onClick={() => onPick(stock)}
                      className="flex min-w-0 flex-1 items-center gap-3 text-left"
                    >
                      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800/80 text-[10px] font-bold text-slate-400">
                        {stock.sector === "ETF" ? "ETF" : marketLabel(stock.market)}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate text-sm font-medium text-slate-100">
                            {stock.name}
                          </span>
                          {stock.is_hot && (
                            <span className="rounded bg-orange-500/15 px-1 py-0.5 text-[9px] text-orange-300">
                              热
                            </span>
                          )}
                        </div>
                        <div className="truncate text-[11px] text-slate-500">
                          {stock.code} · {stock.sector}
                        </div>
                      </div>
                      <div className="shrink-0 text-right">
                        {stock.price != null ? (
                          <>
                            <div className="font-mono text-sm tabular-nums text-slate-200">
                              {formatPrice(stock.price)}
                            </div>
                            {changePct != null && (
                              <div
                                className={cn(
                                  "font-mono text-[11px] tabular-nums",
                                  up && "text-rose-400",
                                  down && "text-emerald-400",
                                  !up && !down && "text-slate-500",
                                )}
                              >
                                {formatPct(changePct)}
                              </div>
                            )}
                          </>
                        ) : (
                          <div className="text-[11px] text-slate-600">—</div>
                        )}
                      </div>
                    </button>
                    <button
                      type="button"
                      onClick={() => setPoolStock(stock)}
                      className={cn(
                        "shrink-0 px-1 text-sm",
                        starred ? "text-amber-400" : "text-slate-600",
                      )}
                      aria-label={starred ? "已在关注" : "入池"}
                    >
                      {starred ? "★" : "☆"}
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {poolStock && (
        <AddToPoolModal stock={poolStock} onClose={() => setPoolStock(null)} />
      )}
    </div>
  );
}

function LinkChip({ to, label }: { to: string; label: string }) {
  return (
    <Link
      to={to}
      className="shrink-0 rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[11px] text-slate-300"
    >
      {label}
    </Link>
  );
}
