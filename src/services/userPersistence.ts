/** 用户态持久化：Rust SQLite 为主，localStorage 仅作一次性迁移源。 */

import { invoke } from "@tauri-apps/api/core";
import type {
  Holding,
  JournalEntry,
  MonitorAlert,
  MonitorRule,
  PoolGroup,
  PoolItem,
  Stock,
  StrategyCompose,
} from "@/types";

export type UserSettings = {
  lookbackDays?: number | null;
  predictMode?: string | null;
  predictHorizonDays?: number | null;
  screenCompose?: StrategyCompose | null;
  monitorEnabled?: boolean | null;
  predictComposeCollapsed?: boolean | null;
  predictComposeHeight?: number | null;
};

export type UserDataSnapshot = {
  watchlist: Stock[];
  strategyMap: Record<string, StrategyCompose>;
  settings: UserSettings;
  monitorRules: MonitorRule[];
  monitorAlerts: MonitorAlert[];
  poolGroups: PoolGroup[];
  poolItems: PoolItem[];
  holdings: Holding[];
  journalEntries: JournalEntry[];
  schemaVersion: number;
  localstorageMigrated: boolean;
};

export type UserDbStatus = {
  schemaVersion: number;
  path: string;
  needsLocalstorageImport: boolean;
  hasUserRows: boolean;
};

export type LegacyLocalStoragePayload = {
  watchlist?: unknown;
  watchlistLegacy?: unknown;
  strategyMap?: Record<string, StrategyCompose>;
  settings?: UserSettings;
  monitorRules?: MonitorRule[];
  monitorAlerts?: MonitorAlert[];
};

const STRATEGY_MAP_KEY = "strategy_compose_by_stock_v1";
const PREDICT_MODE_KEY = "predict_mode_v1";
const PREDICT_HORIZON_KEY = "predict_horizon_days_v1";
const SCREEN_COMPOSE_KEY = "screen_compose_v1";
const WATCHLIST_KEY = "watchlist_v2";
const WATCHLIST_LEGACY_KEY = "watchlist";
const RULES_KEY = "monitor_rules_v1";
const ALERTS_KEY = "monitor_alerts_v1";
const ENABLED_KEY = "monitor_enabled_v1";
const COMPOSE_COLLAPSED_KEY = "predict_compose_collapsed_v1";
const COMPOSE_HEIGHT_KEY = "predict_compose_height_v1";

let persistenceReady = false;

export function isUserPersistenceReady() {
  return persistenceReady;
}

function lsGet(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function lsJson<T>(key: string): T | undefined {
  const raw = lsGet(key);
  if (!raw) return undefined;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return undefined;
  }
}

/** 收集旧版 localStorage，供一次性导入。 */
export function collectLegacyLocalStorage(): LegacyLocalStoragePayload {
  const lookbackRaw = lsGet("lookbackDays");
  const lookbackDays = lookbackRaw ? Number(lookbackRaw) : undefined;
  const horizonRaw = lsGet(PREDICT_HORIZON_KEY);
  const predictHorizonDays = horizonRaw ? Number(horizonRaw) : undefined;
  const heightRaw = lsGet(COMPOSE_HEIGHT_KEY);
  const predictComposeHeight = heightRaw ? Number(heightRaw) : undefined;
  const collapsed = lsGet(COMPOSE_COLLAPSED_KEY);

  return {
    watchlist: lsJson(WATCHLIST_KEY),
    watchlistLegacy: lsJson(WATCHLIST_LEGACY_KEY),
    strategyMap: lsJson(STRATEGY_MAP_KEY),
    settings: {
      lookbackDays: Number.isFinite(lookbackDays) ? lookbackDays : undefined,
      predictMode: lsGet(PREDICT_MODE_KEY) ?? undefined,
      predictHorizonDays: Number.isFinite(predictHorizonDays)
        ? predictHorizonDays
        : undefined,
      screenCompose: lsJson(SCREEN_COMPOSE_KEY),
      monitorEnabled: lsGet(ENABLED_KEY) === "1" ? true : lsGet(ENABLED_KEY) === "0" ? false : undefined,
      predictComposeCollapsed:
        collapsed === "1" ? true : collapsed === "0" ? false : undefined,
      predictComposeHeight: Number.isFinite(predictComposeHeight)
        ? predictComposeHeight
        : undefined,
    },
    monitorRules: lsJson(RULES_KEY),
    monitorAlerts: lsJson(ALERTS_KEY),
  };
}

export async function ensureUserDb() {
  return invoke<UserDbStatus>("ensure_user_db");
}

export async function loadUserData() {
  return invoke<UserDataSnapshot>("load_user_data");
}

export async function saveWatchlist(items: Stock[]) {
  if (!persistenceReady) return;
  await invoke("save_watchlist", { items });
}

export async function savePool(groups: PoolGroup[], items: PoolItem[]) {
  if (!persistenceReady) return;
  await invoke("save_pool", { groups, items });
}

export async function saveHoldings(items: Holding[]) {
  if (!persistenceReady) return;
  await invoke("save_holdings", { items });
}

export async function saveJournalEntries(entries: JournalEntry[]) {
  if (!persistenceReady) return;
  await invoke("save_journal_entries", { entries });
}

export async function saveStrategyMap(map: Record<string, StrategyCompose>) {
  if (!persistenceReady) return;
  await invoke("save_strategy_map", { map });
}

export async function saveUserSettings(settings: UserSettings) {
  if (!persistenceReady) return;
  await invoke("save_user_settings", { settings });
}

export async function saveMonitorRules(rules: MonitorRule[]) {
  if (!persistenceReady) return;
  await invoke("save_monitor_rules", { rules });
}

export async function saveMonitorAlerts(alerts: MonitorAlert[]) {
  if (!persistenceReady) return;
  await invoke("save_monitor_alerts", { alerts });
}

export async function importFromLocalstorage(payload: LegacyLocalStoragePayload) {
  return invoke<UserDataSnapshot>("import_from_localstorage", { payload });
}

export async function markLocalstorageMigrated() {
  await invoke("mark_localstorage_migrated");
}

export async function exportUserData() {
  return invoke<string>("export_user_data");
}

export async function importUserData(json: string) {
  return invoke<UserDataSnapshot>("import_user_data", { json });
}

/**
 * 启动：ensure → 必要时从 LS 导入 → 返回快照。
 * 成功后主路径不再写 localStorage。
 */
export async function bootstrapUserPersistence(): Promise<UserDataSnapshot> {
  const status = await ensureUserDb();
  if (status.needsLocalstorageImport) {
    const payload = collectLegacyLocalStorage();
    const hasAnything =
      payload.watchlist != null ||
      payload.watchlistLegacy != null ||
      (payload.strategyMap && Object.keys(payload.strategyMap).length > 0) ||
      payload.monitorRules?.length ||
      payload.settings?.lookbackDays != null ||
      payload.settings?.predictMode != null ||
      payload.settings?.screenCompose != null;
    if (hasAnything) {
      const snap = await importFromLocalstorage(payload);
      persistenceReady = true;
      return snap;
    }
    await markLocalstorageMigrated();
  }
  const snap = await loadUserData();
  persistenceReady = true;
  return snap;
}

/** 设置页导入后刷新内存状态前调用。 */
export function applyImportedSnapshotFlag() {
  persistenceReady = true;
}
