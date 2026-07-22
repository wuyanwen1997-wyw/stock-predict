import { create } from "zustand";
import {
  analyzeStock,
  defaultScreenCompose,
  defaultStrategyCompose,
  listAlgorithms,
  listStrategySources,
  loadStocks,
  runSmartScreen,
  searchStocks,
} from "@/services/api";
import type {
  AlgorithmInfo,
  BacktestResult,
  DailyBar,
  PredictionResult,
  ScreenFilters,
  ScreenHit,
  ScreenProgressEvent,
  ScreenResult,
  ScreenUniverse,
  Stock,
  StrategyCompose,
  StrategySourceInfo,
} from "@/types";

const STRATEGY_MAP_KEY = "strategy_compose_by_stock_v1";
const PREDICT_MODE_KEY = "predict_mode_v1";
const PREDICT_HORIZON_KEY = "predict_horizon_days_v1";
const SCREEN_COMPOSE_KEY = "screen_compose_v1";

export type PredictMode = "daily" | "trend";

const DEFAULT_SCREEN_FILTERS: ScreenFilters = {
  exclude_st: true,
  min_price: 2,
  min_change_pct: -5,
  max_change_pct: 7,
  main_board_only: false,
};

function loadScreenCompose(): StrategyCompose | null {
  try {
    const raw = localStorage.getItem(SCREEN_COMPOSE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as unknown;
    return isValidCompose(parsed) ? cloneCompose(parsed) : null;
  } catch {
    return null;
  }
}

function saveScreenCompose(compose: StrategyCompose) {
  try {
    localStorage.setItem(SCREEN_COMPOSE_KEY, JSON.stringify(compose));
  } catch {
    /* ignore */
  }
}

const TREND_HORIZONS = [2, 3, 4, 5] as const;

function loadPredictMode(): PredictMode {
  try {
    const v = localStorage.getItem(PREDICT_MODE_KEY);
    if (v === "trend" || v === "daily") return v;
  } catch {
    /* ignore */
  }
  return "daily";
}

function loadHorizonDays(mode: PredictMode): number {
  try {
    const n = Number(localStorage.getItem(PREDICT_HORIZON_KEY));
    if (mode === "daily") return 1;
    if (TREND_HORIZONS.includes(n as (typeof TREND_HORIZONS)[number])) return n;
  } catch {
    /* ignore */
  }
  return mode === "trend" ? 3 : 1;
}

function persistPredictPrefs(mode: PredictMode, horizonDays: number) {
  try {
    localStorage.setItem(PREDICT_MODE_KEY, mode);
    // 只持久化多日跨度，避免切回单日时覆盖用户上次选的 2–5 日
    if (mode === "trend" && horizonDays >= 2) {
      localStorage.setItem(PREDICT_HORIZON_KEY, String(horizonDays));
    }
  } catch {
    /* ignore */
  }
}

/** 统一用 6 位数字代码做存储键，避免 SH510980 / 510980 不一致 */
function composeKey(code: string): string {
  const digits = code.replace(/\D/g, "");
  if (digits.length >= 6) return digits.slice(-6);
  if (digits.length > 0) return digits.padStart(6, "0");
  return code.trim();
}

function cloneCompose(c: StrategyCompose): StrategyCompose {
  return {
    lookback_days: c.lookback_days,
    sources: c.sources.map((s) => ({ ...s })),
  };
}

function isValidCompose(v: unknown): v is StrategyCompose {
  if (!v || typeof v !== "object") return false;
  const c = v as StrategyCompose;
  return (
    typeof c.lookback_days === "number" &&
    Array.isArray(c.sources) &&
    c.sources.every(
      (s) =>
        s &&
        typeof s.id === "string" &&
        typeof s.enabled === "boolean" &&
        typeof s.weight === "number",
    )
  );
}

/** 用默认目录补齐缺失信号源，保留用户已设启用/权重 */
function mergeWithDefault(
  saved: StrategyCompose | null | undefined,
  defaultCompose: StrategyCompose | null,
  fallbackLookback: number,
): StrategyCompose {
  if (!defaultCompose) {
    return saved
      ? cloneCompose(saved)
      : { lookback_days: fallbackLookback, sources: [] };
  }
  if (!saved) {
    const c = cloneCompose(defaultCompose);
    c.lookback_days = fallbackLookback;
    return c;
  }

  const byId = new Map(saved.sources.map((s) => [s.id, s]));
  const sources = defaultCompose.sources.map((def) => {
    const prev = byId.get(def.id);
    return prev
      ? { id: def.id, enabled: prev.enabled, weight: prev.weight }
      : { ...def };
  });
  // 保留默认里没有、但用户旧配置里有的源
  for (const s of saved.sources) {
    if (!sources.some((x) => x.id === s.id)) sources.push({ ...s });
  }

  return {
    lookback_days: saved.lookback_days || fallbackLookback,
    sources,
  };
}

function loadStrategyMap(): Record<string, StrategyCompose> {
  try {
    const raw = localStorage.getItem(STRATEGY_MAP_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (!parsed || typeof parsed !== "object") return {};

    const out: Record<string, StrategyCompose> = {};
    for (const [k, v] of Object.entries(parsed)) {
      if (!isValidCompose(v)) continue;
      const key = composeKey(k);
      // 同一股票多种键时，后写覆盖前写；优先保留更完整的
      if (!out[key] || v.sources.length >= out[key].sources.length) {
        out[key] = cloneCompose(v);
      }
    }
    return out;
  } catch {
    return {};
  }
}

/** 读盘合并后再写，避免内存 map 为空时覆盖掉其它股票配置 */
function upsertStrategyCompose(
  code: string,
  compose: StrategyCompose,
  memoryMap: Record<string, StrategyCompose>,
): Record<string, StrategyCompose> {
  const key = composeKey(code);
  const merged = {
    ...loadStrategyMap(),
    ...memoryMap,
    [key]: cloneCompose(compose),
  };
  // 去掉未规范化的重复键
  const normalized: Record<string, StrategyCompose> = {};
  for (const [k, v] of Object.entries(merged)) {
    const nk = composeKey(k);
    normalized[nk] = v;
  }
  try {
    localStorage.setItem(STRATEGY_MAP_KEY, JSON.stringify(normalized));
  } catch (e) {
    console.warn("保存信号组合失败", e);
  }
  return normalized;
}

let searchTimer: ReturnType<typeof setTimeout> | null = null;
let searchSeq = 0;

interface StockState {
  stocks: Stock[];
  hotStocks: Stock[];
  searchResults: Stock[];
  searchQuery: string;
  searching: boolean;
  algorithms: AlgorithmInfo[];
  strategySources: StrategySourceInfo[];
  defaultCompose: StrategyCompose | null;
  /** 按股票代码保存的组合配置 */
  strategyMap: Record<string, StrategyCompose>;
  selectedStock: Stock | null;
  activeAlgorithm: string;
  lookbackDays: number;
  /** daily=次日，trend=2–5 日累计趋势 */
  predictMode: PredictMode;
  /** 1 或 2–5 */
  horizonDays: number;
  prediction: PredictionResult | null;
  klines: DailyBar[];
  backtest: BacktestResult | null;
  watchlist: Stock[];
  loading: boolean;
  predicting: boolean;
  loadingKlines: boolean;
  loadingBacktest: boolean;
  error: string;
  analysisSeq: number;

  init: () => Promise<void>;
  selectStock: (stock: Stock) => void;
  setAlgorithm: (id: string) => void;
  setLookbackDays: (days: number) => void;
  setPredictMode: (mode: PredictMode) => void;
  setHorizonDays: (days: number) => void;
  getComposeForStock: (code: string) => StrategyCompose;
  updateCompose: (patch: Partial<StrategyCompose> | ((c: StrategyCompose) => StrategyCompose)) => void;
  toggleSource: (sourceId: string) => void;
  setSourceWeight: (sourceId: string, weight: number) => void;
  resetComposeForStock: () => void;
  /** 应用按标的调优的推荐组合（宽基：多因子70+消息30，以整体准确率为准） */
  applyTunedComposeForStock: () => void;
  runPrediction: () => Promise<void>;
  loadStockAnalysis: () => Promise<void>;
  toggleWatchlist: (stock: Stock) => void;
  setSearchQuery: (query: string) => void;
  runSearch: () => Promise<void>;
  clearSearch: () => void;

  /** 智能选股 */
  screenUniverse: ScreenUniverse;
  screenFilters: ScreenFilters;
  screenCompose: StrategyCompose | null;
  screenHorizonDays: number;
  screenTopN: number;
  screenResult: ScreenResult | null;
  screening: boolean;
  screenProgress: { done: number; total: number; code: string };
  setScreenUniverse: (u: ScreenUniverse) => void;
  setScreenFilters: (patch: Partial<ScreenFilters>) => void;
  setScreenHorizonDays: (days: number) => void;
  setScreenTopN: (n: number) => void;
  updateScreenCompose: (
    patch: Partial<StrategyCompose> | ((c: StrategyCompose) => StrategyCompose),
  ) => void;
  toggleScreenSource: (sourceId: string) => void;
  setScreenSourceWeight: (sourceId: string, weight: number) => void;
  resetScreenCompose: () => Promise<void>;
  runSmartScreen: () => Promise<void>;
  applyScreenHit: (hit: ScreenHit) => void;
}

export const useStockStore = create<StockState>((set, get) => ({
  stocks: [],
  hotStocks: [],
  searchResults: [],
  searchQuery: "",
  searching: false,
  algorithms: [],
  strategySources: [],
  defaultCompose: null,
  strategyMap: {},
  selectedStock: null,
  activeAlgorithm: "compose",
  lookbackDays: 50,
  predictMode: loadPredictMode(),
  horizonDays: loadHorizonDays(loadPredictMode()),
  prediction: null,
  klines: [],
  backtest: null,
  watchlist: [],
  loading: false,
  predicting: false,
  loadingKlines: false,
  loadingBacktest: false,
  error: "",
  analysisSeq: 0,

  screenUniverse: "mixed",
  screenFilters: { ...DEFAULT_SCREEN_FILTERS },
  screenCompose: loadScreenCompose(),
  screenHorizonDays: 1,
  screenTopN: 20,
  screenResult: null,
  screening: false,
  screenProgress: { done: 0, total: 0, code: "" },

  init: async () => {
    set({ loading: true, error: "" });
    try {
      const settled = await Promise.allSettled([
        loadStocks(),
        listAlgorithms(),
        listStrategySources(),
        defaultStrategyCompose(),
        defaultScreenCompose(),
      ]);

      const stocksPayload =
        settled[0].status === "fulfilled" ? settled[0].value : { stocks: [], hot_stocks: [] };
      const stocks = stocksPayload.stocks;
      const hotStocksFromApi = stocksPayload.hot_stocks ?? [];
      const hotWarning = stocksPayload.warning?.trim() || "";
      const algorithms = settled[1].status === "fulfilled" ? settled[1].value : [];
      const strategySources = settled[2].status === "fulfilled" ? settled[2].value : [];
      const defaultCompose = settled[3].status === "fulfilled" ? settled[3].value : null;
      const screenDefault =
        settled[4].status === "fulfilled" ? settled[4].value : null;

      const labels = ["股票列表", "算法列表", "信号源", "默认组合", "选股组合"] as const;
      const failures = settled
        .map((r, i) =>
          r.status === "rejected" ? `${labels[i]}: ${String(r.reason)}` : null,
        )
        .filter((x): x is string => Boolean(x));

      if (stocks.length === 0) {
        throw new Error(failures[0] ?? "股票列表为空");
      }

      const softErrors = [
        ...failures.filter((f) => !f.startsWith("股票列表")),
        ...(hotWarning ? [hotWarning] : []),
      ];

      if (hotWarning) {
        console.warn("[stock-predict] hot list warning:", hotWarning);
      }

      let watchlist = loadWatchlist([...stocks, ...hotStocksFromApi]);
      watchlist = await recoverLegacyWatchlist(watchlist);
      const strategyMap = loadStrategyMap();
      const selectedStock =
        (hotStocksFromApi[0] ?? stocks.find((s) => s.is_hot) ?? stocks[0]) ?? null;

      const savedLookback = Number(localStorage.getItem("lookbackDays"));
      let lookbackDays = [25, 50, 60, 90, 120].includes(savedLookback) ? savedLookback : 50;
      if (selectedStock) {
        const saved = strategyMap[composeKey(selectedStock.code)];
        if (saved?.lookback_days) lookbackDays = saved.lookback_days;
      }

      const existingScreen = get().screenCompose;
      const screenCompose = existingScreen
        ? mergeWithDefault(existingScreen, screenDefault, 50)
        : screenDefault
          ? cloneCompose(screenDefault)
          : null;
      if (screenCompose) saveScreenCompose(screenCompose);

      set({
        stocks,
        hotStocks: hotStocksFromApi.length > 0 ? hotStocksFromApi : stocks.filter((s) => s.is_hot),
        algorithms,
        strategySources,
        defaultCompose,
        strategyMap,
        selectedStock,
        watchlist,
        lookbackDays,
        screenCompose,
        loading: false,
        error: softErrors.length > 0 ? softErrors.join("；") : "",
      });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  selectStock: (stock) => {
    const compose = get().getComposeForStock(stock.code);
    set({
      selectedStock: stock,
      prediction: null,
      klines: [],
      backtest: null,
      lookbackDays: compose.lookback_days,
    });
    void get().runPrediction();
  },

  setAlgorithm: (id) => {
    set({ activeAlgorithm: id });
    if (get().selectedStock) void get().runPrediction();
  },

  setLookbackDays: (days) => {
    localStorage.setItem("lookbackDays", String(days));
    set({ lookbackDays: days });
    if (get().selectedStock) {
      get().updateCompose({ lookback_days: days });
    }
  },

  setPredictMode: (mode) => {
    const prevHorizon = get().horizonDays;
    let horizonDays = 1;
    if (mode === "trend") {
      horizonDays =
        prevHorizon >= 2 && prevHorizon <= 5
          ? prevHorizon
          : loadHorizonDays("trend");
      if (horizonDays < 2) horizonDays = 3;
    }
    persistPredictPrefs(mode, horizonDays);
    set({ predictMode: mode, horizonDays });
    if (get().selectedStock) void get().runPrediction();
  },

  setHorizonDays: (days) => {
    const mode = get().predictMode;
    const horizonDays =
      mode === "daily" ? 1 : Math.min(5, Math.max(2, Math.round(days)));
    persistPredictPrefs(mode, horizonDays);
    set({ horizonDays });
    if (get().selectedStock) void get().runPrediction();
  },

  getComposeForStock: (code) => {
    const { strategyMap, defaultCompose, lookbackDays } = get();
    const key = composeKey(code);
    const saved = strategyMap[key] ?? loadStrategyMap()[key];
    return mergeWithDefault(saved, defaultCompose, lookbackDays);
  },

  updateCompose: (patch) => {
    const stock = get().selectedStock;
    if (!stock) return;
    const current = get().getComposeForStock(stock.code);
    const next =
      typeof patch === "function"
        ? patch(current)
        : { ...current, ...patch, sources: patch.sources ?? current.sources };
    const strategyMap = upsertStrategyCompose(stock.code, next, get().strategyMap);
    set({
      strategyMap,
      lookbackDays: next.lookback_days,
    });
    void get().runPrediction();
  },

  toggleSource: (sourceId) => {
    get().updateCompose((c) => ({
      ...c,
      sources: c.sources.map((s) =>
        s.id === sourceId ? { ...s, enabled: !s.enabled } : s,
      ),
    }));
  },

  setSourceWeight: (sourceId, weight) => {
    get().updateCompose((c) => ({
      ...c,
      sources: c.sources.map((s) =>
        s.id === sourceId ? { ...s, weight: Math.max(0, Math.min(100, weight)) } : s,
      ),
    }));
  },

  resetComposeForStock: () => {
    const stock = get().selectedStock;
    const def = get().defaultCompose;
    if (!stock || !def) return;
    const next = cloneCompose(def);
    next.lookback_days = get().lookbackDays;
    const strategyMap = upsertStrategyCompose(stock.code, next, get().strategyMap);
    set({ strategyMap });
    void get().runPrediction();
  },

  applyTunedComposeForStock: () => {
    const stock = get().selectedStock;
    if (!stock) return;
    void (async () => {
      try {
        const { defaultStrategyComposeForStock } = await import("@/services/api");
        const tuned = await defaultStrategyComposeForStock(stock);
        const strategyMap = upsertStrategyCompose(stock.code, tuned, get().strategyMap);
        set({
          strategyMap,
          lookbackDays: tuned.lookback_days,
        });
        localStorage.setItem("lookbackDays", String(tuned.lookback_days));
        void get().runPrediction();
      } catch (e) {
        set({ error: `应用调优组合失败: ${String(e)}` });
      }
    })();
  },

  runPrediction: async () => {
    const { selectedStock, activeAlgorithm, lookbackDays, horizonDays } = get();
    if (!selectedStock) return;

    const compose = get().getComposeForStock(selectedStock.code);
    compose.lookback_days = lookbackDays;

    const seq = get().analysisSeq + 1;
    set({
      analysisSeq: seq,
      predicting: true,
      loadingKlines: true,
      loadingBacktest: true,
      error: "",
    });

    try {
      const result = await analyzeStock(
        selectedStock,
        activeAlgorithm,
        lookbackDays,
        compose,
        horizonDays,
      );
      if (get().analysisSeq !== seq) return;

      set({
        prediction: result.prediction,
        klines: result.klines,
        backtest: result.backtest,
        predicting: false,
        loadingKlines: false,
        loadingBacktest: false,
      });
    } catch (err) {
      if (get().analysisSeq !== seq) return;
      set({
        error: String(err),
        predicting: false,
        loadingKlines: false,
        loadingBacktest: false,
      });
    }
  },

  loadStockAnalysis: async () => {
    await get().runPrediction();
  },

  toggleWatchlist: (stock) => {
    const exists = get().watchlist.some((s) => s.code === stock.code);
    const next = exists
      ? get().watchlist.filter((s) => s.code !== stock.code)
      : [...get().watchlist, stock];
    saveWatchlist(next);
    set({ watchlist: next });
  },

  setSearchQuery: (query) => {
    const q = query.trim().toLowerCase();
    set({ searchQuery: query });

    if (!q) {
      searchSeq += 1;
      if (searchTimer) {
        clearTimeout(searchTimer);
        searchTimer = null;
      }
      set({ searchResults: [], searching: false });
      return;
    }

    // Instant local matches (stocks + ETF in bundled list)
    const local = get()
      .stocks.filter((s) => {
        const code = s.code.toLowerCase();
        const name = s.name.toLowerCase();
        const sector = s.sector.toLowerCase();
        return code.includes(q) || name.includes(q) || sector.includes(q);
      })
      .slice(0, 20);
    set({ searchResults: local, searching: true });

    if (searchTimer) clearTimeout(searchTimer);
    const seq = ++searchSeq;
    searchTimer = setTimeout(() => {
      void (async () => {
        try {
          const remote = await searchStocks(query.trim(), 12);
          if (seq !== searchSeq) return;
          const seen = new Set(remote.map((s) => s.code));
          const merged = [
            ...remote,
            ...local.filter((s) => !seen.has(s.code)),
          ].slice(0, 24);
          set({ searchResults: merged, searching: false });
        } catch (err) {
          if (seq !== searchSeq) return;
          // Keep local results; soft-fail remote
          set({ searching: false, error: String(err) });
        }
      })();
    }, 280);
  },

  runSearch: async () => {
    const q = get().searchQuery.trim();
    if (!q) {
      set({ searchResults: [], searching: false });
      return;
    }

    // Force immediate remote refresh (Enter / 搜 button)
    searchSeq += 1;
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
    const seq = ++searchSeq;
    set({ searching: true, error: "" });
    try {
      const remote = await searchStocks(q, 12);
      if (seq !== searchSeq) return;
      const local = get().stocks.filter((s) => {
        const t = q.toLowerCase();
        return (
          s.code.toLowerCase().includes(t) ||
          s.name.toLowerCase().includes(t) ||
          s.sector.toLowerCase().includes(t)
        );
      });
      const seen = new Set(remote.map((s) => s.code));
      const merged = [...remote, ...local.filter((s) => !seen.has(s.code))].slice(0, 24);
      set({ searchResults: merged, searching: false });
    } catch (err) {
      if (seq !== searchSeq) return;
      set({ error: String(err), searching: false });
    }
  },

  clearSearch: () => {
    searchSeq += 1;
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
    set({ searchQuery: "", searchResults: [], searching: false });
  },

  setScreenUniverse: (u) => set({ screenUniverse: u }),

  setScreenFilters: (patch) =>
    set({ screenFilters: { ...get().screenFilters, ...patch } }),

  setScreenHorizonDays: (days) =>
    set({ screenHorizonDays: Math.min(5, Math.max(1, Math.round(days))) }),

  setScreenTopN: (n) => set({ screenTopN: Math.min(50, Math.max(5, Math.round(n))) }),

  updateScreenCompose: (patch) => {
    const current = get().screenCompose;
    if (!current) return;
    const next =
      typeof patch === "function"
        ? patch(current)
        : { ...current, ...patch, sources: patch.sources ?? current.sources };
    saveScreenCompose(next);
    set({ screenCompose: next });
  },

  toggleScreenSource: (sourceId) => {
    get().updateScreenCompose((c) => ({
      ...c,
      sources: c.sources.map((s) =>
        s.id === sourceId ? { ...s, enabled: !s.enabled } : s,
      ),
    }));
  },

  setScreenSourceWeight: (sourceId, weight) => {
    get().updateScreenCompose((c) => ({
      ...c,
      sources: c.sources.map((s) =>
        s.id === sourceId ? { ...s, weight: Math.max(0, Math.min(100, weight)) } : s,
      ),
    }));
  },

  resetScreenCompose: async () => {
    try {
      const def = await defaultScreenCompose();
      saveScreenCompose(def);
      set({ screenCompose: def });
    } catch (e) {
      set({ error: `重置选股组合失败: ${String(e)}` });
    }
  },

  runSmartScreen: async () => {
    if (get().screening) return;
    set({
      screening: true,
      screenResult: null,
      screenProgress: { done: 0, total: 0, code: "" },
      error: "",
    });

    let unlisten: (() => void) | undefined;
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<ScreenProgressEvent>("smart-screen-progress", (event) => {
        set({
          screenProgress: {
            done: event.payload.done,
            total: event.payload.total,
            code: event.payload.code,
          },
        });
      });
    } catch {
      /* 非 Tauri 环境忽略 */
    }

    try {
      const {
        screenUniverse,
        screenFilters,
        screenCompose,
        screenHorizonDays,
        screenTopN,
        watchlist,
      } = get();
      const compose = screenCompose ?? (await defaultScreenCompose());
      const result = await runSmartScreen({
        universe: screenUniverse,
        watchlist,
        filters: screenFilters,
        compose,
        horizon_days: screenHorizonDays,
        lookback_days: compose.lookback_days || 50,
        top_n: screenTopN,
        concurrency: 4,
      });
      set({ screenResult: result, screening: false });
    } catch (err) {
      set({ error: String(err), screening: false });
    } finally {
      unlisten?.();
    }
  },

  applyScreenHit: (hit) => {
    get().selectStock(hit.stock);
  },
}));

const WATCHLIST_KEY = "watchlist_v2";
const WATCHLIST_LEGACY_KEY = "watchlist";

function loadWatchlist(known: Stock[]): Stock[] {
  try {
    const raw = localStorage.getItem(WATCHLIST_KEY) ?? localStorage.getItem(WATCHLIST_LEGACY_KEY);
    if (!raw) return [];

    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0) return [];

    if (typeof parsed[0] === "string") {
      return (parsed as string[])
        .map((code) => known.find((s) => s.code === code))
        .filter((s): s is Stock => s != null);
    }

    return parsed as Stock[];
  } catch {
    return [];
  }
}

function saveWatchlist(items: Stock[]) {
  localStorage.setItem(WATCHLIST_KEY, JSON.stringify(items));
}

async function recoverLegacyWatchlist(current: Stock[]): Promise<Stock[]> {
  try {
    const raw = localStorage.getItem(WATCHLIST_LEGACY_KEY);
    if (!raw) return current;

    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0 || typeof parsed[0] !== "string") {
      return current;
    }

    const legacyCodes = parsed as string[];
    const knownCodes = new Set(current.map((s) => s.code));
    const missing = legacyCodes.filter((code) => !knownCodes.has(code));
    if (missing.length === 0) return current;

    const recovered: Stock[] = [];
    for (const code of missing) {
      try {
        const results = await searchStocks(code, 8);
        const match = results.find((s) => s.code === code) ?? results[0];
        if (match) recovered.push(match);
      } catch {
        // ignore
      }
    }

    if (recovered.length === 0) return current;

    const merged = [...current];
    for (const stock of recovered) {
      if (!merged.some((s) => s.code === stock.code)) {
        merged.push(stock);
      }
    }
    saveWatchlist(merged);
    return merged;
  } catch {
    return current;
  }
}
