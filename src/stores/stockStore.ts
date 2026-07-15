import { create } from "zustand";
import {
  analyzeStock,
  defaultStrategyCompose,
  listAlgorithms,
  listStrategySources,
  loadStocks,
  searchStocks,
} from "@/services/api";
import type {
  AlgorithmInfo,
  BacktestResult,
  DailyBar,
  PredictionResult,
  Stock,
  StrategyCompose,
  StrategySourceInfo,
} from "@/types";

const STRATEGY_MAP_KEY = "strategy_compose_by_stock_v1";

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
  getComposeForStock: (code: string) => StrategyCompose;
  updateCompose: (patch: Partial<StrategyCompose> | ((c: StrategyCompose) => StrategyCompose)) => void;
  toggleSource: (sourceId: string) => void;
  setSourceWeight: (sourceId: string, weight: number) => void;
  resetComposeForStock: () => void;
  runPrediction: () => Promise<void>;
  loadStockAnalysis: () => Promise<void>;
  toggleWatchlist: (stock: Stock) => void;
  setSearchQuery: (query: string) => void;
  runSearch: () => Promise<void>;
  clearSearch: () => void;
}

function cloneCompose(c: StrategyCompose): StrategyCompose {
  return {
    lookback_days: c.lookback_days,
    sources: c.sources.map((s) => ({ ...s })),
  };
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

  init: async () => {
    set({ loading: true, error: "" });
    try {
      const [{ stocks, hot_stocks: hotStocks }, algorithms, strategySources, defaultCompose] =
        await Promise.all([
          loadStocks(),
          listAlgorithms(),
          listStrategySources(),
          defaultStrategyCompose(),
        ]);
      const hotStocksFromApi = hotStocks;
      let watchlist = loadWatchlist([...stocks, ...hotStocksFromApi]);
      watchlist = await recoverLegacyWatchlist(watchlist);
      const strategyMap = loadStrategyMap();
      const selectedStock =
        (hotStocksFromApi[0] ?? stocks.find((s) => s.is_hot) ?? stocks[0]) ?? null;

      const savedLookback = Number(localStorage.getItem("lookbackDays"));
      let lookbackDays = [25, 50, 60, 90, 120].includes(savedLookback) ? savedLookback : 50;
      if (selectedStock && strategyMap[selectedStock.code]?.lookback_days) {
        lookbackDays = strategyMap[selectedStock.code].lookback_days;
      }

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
        loading: false,
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

  getComposeForStock: (code) => {
    const { strategyMap, defaultCompose, lookbackDays } = get();
    const saved = strategyMap[code];
    if (saved) return cloneCompose(saved);
    if (defaultCompose) {
      const c = cloneCompose(defaultCompose);
      c.lookback_days = lookbackDays;
      return c;
    }
    return { lookback_days: lookbackDays, sources: [] };
  },

  updateCompose: (patch) => {
    const stock = get().selectedStock;
    if (!stock) return;
    const current = get().getComposeForStock(stock.code);
    const next =
      typeof patch === "function" ? patch(current) : { ...current, ...patch, sources: patch.sources ?? current.sources };
    const strategyMap = { ...get().strategyMap, [stock.code]: cloneCompose(next) };
    saveStrategyMap(strategyMap);
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
    const strategyMap = { ...get().strategyMap, [stock.code]: next };
    saveStrategyMap(strategyMap);
    set({ strategyMap });
    void get().runPrediction();
  },

  runPrediction: async () => {
    const { selectedStock, activeAlgorithm, lookbackDays } = get();
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

  setSearchQuery: (query) => set({ searchQuery: query }),

  runSearch: async () => {
    const q = get().searchQuery.trim();
    if (!q) {
      set({ searchResults: [] });
      return;
    }

    set({ searching: true, error: "" });
    try {
      const searchResults = await searchStocks(q, 12);
      set({ searchResults, searching: false });
    } catch (err) {
      set({ error: String(err), searching: false });
    }
  },

  clearSearch: () => set({ searchQuery: "", searchResults: [] }),
}));

function loadStrategyMap(): Record<string, StrategyCompose> {
  try {
    const raw = localStorage.getItem(STRATEGY_MAP_KEY);
    if (!raw) return {};
    return JSON.parse(raw) as Record<string, StrategyCompose>;
  } catch {
    return {};
  }
}

function saveStrategyMap(map: Record<string, StrategyCompose>) {
  localStorage.setItem(STRATEGY_MAP_KEY, JSON.stringify(map));
}

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
