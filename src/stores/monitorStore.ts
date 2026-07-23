import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import {
  isServiceRunning,
  startService,
  stopService,
} from "tauri-plugin-background-service";
import { monitorSyncConfig } from "@/services/api";
import {
  saveMonitorAlerts,
  saveMonitorRules,
  saveUserSettings,
  saveWatchlist as persistWatchlist,
} from "@/services/userPersistence";
import type { AlertCondition, MonitorAlert, MonitorQuoteEvent, MonitorRule, Stock } from "@/types";
import { useStockStore } from "@/stores/stockStore";

const MAX_ALERTS = 50;
const INTERVAL_SECS = 15;

function newRuleId() {
  return `r-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

type MonitorState = {
  rules: MonitorRule[];
  alerts: MonitorAlert[];
  wantEnabled: boolean;
  running: boolean;
  starting: boolean;
  error: string | null;
  listenersBound: boolean;
  ensureListeners: () => Promise<void>;
  upsertRule: (rule: Omit<MonitorRule, "id" | "created_at"> & { id?: string }) => void;
  removeRule: (id: string) => void;
  rulesForCode: (code: string) => MonitorRule[];
  clearAlerts: () => void;
  setMonitoring: (on: boolean) => Promise<void>;
  syncAndMaybeRestart: () => Promise<void>;
  ensureNotificationPermission: () => Promise<boolean>;
};

const listenerHandles: UnlistenFn[] = [];

export function conditionSummary(c: AlertCondition): string {
  switch (c.kind) {
    case "price_above":
      return `价格 ≥ ${c.value.toFixed(2)}`;
    case "price_below":
      return `价格 ≤ ${c.value.toFixed(2)}`;
    case "change_pct_above":
      return `涨跌幅 ≥ ${c.value.toFixed(2)}%`;
    case "change_pct_below":
      return `涨跌幅 ≤ ${c.value.toFixed(2)}%`;
  }
}

export const useMonitorStore = create<MonitorState>((set, get) => ({
  rules: [],
  alerts: [],
  wantEnabled: false,
  running: false,
  starting: false,
  error: null,
  listenersBound: false,

  ensureListeners: async () => {
    if (get().listenersBound) return;

    listenerHandles.push(
      await listen<MonitorAlert>("monitor-alert", (ev) => {
        const alert = ev.payload;
        const next = [alert, ...get().alerts].slice(0, MAX_ALERTS);
        void saveMonitorAlerts(next);
        set({ alerts: next });
      }),
    );

    listenerHandles.push(
      await listen<MonitorQuoteEvent[]>("monitor-quotes", (ev) => {
        const events = ev.payload ?? [];
        if (!events.length) return;
        const map = new Map(events.map((e) => [e.code, e]));
        const watchlist = useStockStore.getState().watchlist;
        let changed = false;
        const next = watchlist.map((s) => {
          const q = map.get(s.code);
          if (!q) return s;
          changed = true;
          return {
            ...s,
            price: q.price ?? s.price,
            change_pct: q.changePct ?? s.change_pct,
          };
        });
        if (changed) {
          useStockStore.setState({ watchlist: next });
          void persistWatchlist(next);
        }
      }),
    );

    set({ listenersBound: true });

    try {
      const running = await isServiceRunning();
      set({ running });
      if (get().wantEnabled && !running) {
        void get().setMonitoring(true);
      }
    } catch {
      /* 非 Tauri 环境忽略 */
    }
  },

  upsertRule: (partial) => {
    const rules = [...get().rules];
    const now = new Date().toISOString();
    if (partial.id) {
      const idx = rules.findIndex((r) => r.id === partial.id);
      if (idx >= 0) {
        rules[idx] = { ...rules[idx], ...partial, id: partial.id };
      } else {
        rules.push({
          id: partial.id,
          code: partial.code,
          name: partial.name,
          enabled: partial.enabled,
          condition: partial.condition,
          cooldown_sec: partial.cooldown_sec ?? 300,
          max_per_day: partial.max_per_day ?? 5,
          created_at: now,
        });
      }
    } else {
      rules.push({
        id: newRuleId(),
        code: partial.code,
        name: partial.name,
        enabled: partial.enabled,
        condition: partial.condition,
        cooldown_sec: partial.cooldown_sec ?? 300,
        max_per_day: partial.max_per_day ?? 5,
        created_at: now,
      });
    }
    void saveMonitorRules(rules);
    set({ rules });
    void get().syncAndMaybeRestart();
  },

  removeRule: (id) => {
    const rules = get().rules.filter((r) => r.id !== id);
    void saveMonitorRules(rules);
    set({ rules });
    void get().syncAndMaybeRestart();
  },

  rulesForCode: (code) => get().rules.filter((r) => r.code === code),

  clearAlerts: () => {
    void saveMonitorAlerts([]);
    set({ alerts: [] });
  },

  ensureNotificationPermission: async () => {
    try {
      let granted = await isPermissionGranted();
      if (!granted) {
        const perm = await requestPermission();
        granted = perm === "granted";
      }
      return granted;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  syncAndMaybeRestart: async () => {
    const { rules, running, wantEnabled } = get();
    const stocks: Stock[] = useStockStore.getState().watchlist;
    try {
      await monitorSyncConfig({
        stocks,
        rules,
        interval_secs: INTERVAL_SECS,
        enabled: running || wantEnabled,
      });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  setMonitoring: async (on) => {
    set({ starting: true, error: null });
    try {
      await get().ensureListeners();

      if (on) {
        const ok = await get().ensureNotificationPermission();
        if (!ok) {
          set({
            error: "需要通知权限才能锁屏提醒，请在系统设置中允许通知",
            starting: false,
          });
          return;
        }

        const stocks = useStockStore.getState().watchlist;
        if (stocks.length === 0) {
          set({ error: "请先添加自选股", starting: false });
          return;
        }

        await monitorSyncConfig({
          stocks,
          rules: get().rules,
          interval_secs: INTERVAL_SECS,
          enabled: true,
        });

        await startService({
          serviceLabel: "以太测 · 盯盘中",
          foregroundServiceType: "dataSync",
        });
        void saveUserSettings({ monitorEnabled: true });
        set({ running: true, wantEnabled: true, starting: false });
      } else {
        try {
          await stopService();
        } catch {
          /* already stopped */
        }
        await monitorSyncConfig({
          stocks: useStockStore.getState().watchlist,
          rules: get().rules,
          interval_secs: INTERVAL_SECS,
          enabled: false,
        });
        void saveUserSettings({ monitorEnabled: false });
        set({ running: false, wantEnabled: false, starting: false });
      }
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        starting: false,
        running: false,
      });
      void saveUserSettings({ monitorEnabled: false });
      set({ wantEnabled: false });
    }
  },
}));
