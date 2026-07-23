import type { Holding, PoolGroup, PoolItem, Stock } from "@/types";

export const GROUP_WATCH = "g_watch";
export const GROUP_BUY = "g_buy";
export const GROUP_OBSERVE = "g_observe";
export const GROUP_REMOVED = "g_removed";
export const GROUP_HOLDINGS_MIRROR = "g_holdings_mirror";

export const DEFAULT_POOL_GROUPS: PoolGroup[] = [
  { id: GROUP_WATCH, name: "关注", sortOrder: 0, kind: "user" },
  { id: GROUP_BUY, name: "待买", sortOrder: 1, kind: "user" },
  { id: GROUP_OBSERVE, name: "观察", sortOrder: 2, kind: "user" },
  { id: GROUP_REMOVED, name: "已剔除", sortOrder: 3, kind: "user" },
  { id: GROUP_HOLDINGS_MIRROR, name: "持仓镜像", sortOrder: 99, kind: "mirror" },
];

export function stockFromPoolItem(item: PoolItem): Stock {
  let price: number | undefined;
  let change_pct: number | undefined;
  let is_hot: boolean | undefined;
  try {
    const extra = JSON.parse(item.extraJson || "{}") as {
      price?: number;
      change_pct?: number;
      is_hot?: boolean;
    };
    price = extra.price;
    change_pct = extra.change_pct;
    is_hot = extra.is_hot;
  } catch {
    /* ignore */
  }
  return {
    code: item.code,
    name: item.name,
    market: item.market,
    sector: item.sector,
    price,
    change_pct,
    is_hot,
  };
}

export function poolItemFromStock(
  stock: Stock,
  groupId: string,
  sortOrder = 0,
): PoolItem {
  const extra = {
    price: stock.price,
    change_pct: stock.change_pct,
    is_hot: stock.is_hot,
  };
  return {
    code: stock.code,
    groupId,
    name: stock.name,
    market: stock.market,
    sector: stock.sector,
    sortOrder,
    extraJson: JSON.stringify(extra),
  };
}

export function watchlistFromPool(items: PoolItem[]): Stock[] {
  return items
    .filter((i) => i.groupId === GROUP_WATCH)
    .sort((a, b) => a.sortOrder - b.sortOrder)
    .map(stockFromPoolItem);
}

export function holdingsMirrorItems(holdings: Holding[]): PoolItem[] {
  return holdings.map((h, i) => ({
    code: h.code,
    groupId: GROUP_HOLDINGS_MIRROR,
    name: h.name,
    market: h.market,
    sector: h.sector,
    sortOrder: i,
    extraJson: "{}",
  }));
}

export function newGroupId(): string {
  return `g_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 6)}`;
}

export function newJournalId(): string {
  return `j_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export function todayIsoDate(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function floatingPnl(cost: number, qty: number, price: number | undefined) {
  if (price == null || !Number.isFinite(price)) return null;
  const pnl = (price - cost) * qty;
  const pct = cost > 0 ? ((price - cost) / cost) * 100 : 0;
  return { pnl, pct, price };
}
