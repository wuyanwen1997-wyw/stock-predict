import { create } from "zustand";
import {
  analyzeStock,
  defaultScreenCompose,
  defaultStrategyCompose,
  getStockKlines,
  listAlgorithms,
  listStrategySources,
  loadStocks,
  runSmartScreen,
  searchStocks,
} from "@/services/api";
import {
  bootstrapUserPersistence,
  saveHoldings,
  saveJournalEntries,
  savePool,
  saveStrategyMap as persistStrategyMap,
  saveUserSettings,
  saveWatchlist as persistWatchlist,
  type UserDataSnapshot,
} from "@/services/userPersistence";
import { chartBarLimit } from "@/lib/klineData";
import {
  DEFAULT_POOL_GROUPS,
  GROUP_HOLDINGS_MIRROR,
  GROUP_REMOVED,
  GROUP_WATCH,
  holdingsMirrorItems,
  poolItemFromStock,
  watchlistFromPool,
} from "@/lib/pool";
import type {
  AlgorithmInfo,
  BacktestResult,
  BsMarker,
  DailyBar,
  Holding,
  JournalEntry,
  KlinePeriod,
  PoolGroup,
  PoolItem,
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
import { useMonitorStore } from "@/stores/monitorStore";

const FIXED_POOL_GROUP_IDS = new Set(DEFAULT_POOL_GROUPS.map((g) => g.id));

export type PredictMode = "daily" | "trend";

const DEFAULT_SCREEN_FILTERS: ScreenFilters = {
  exclude_st: true,
  min_price: 2,
  min_change_pct: -5,
  max_change_pct: 7,
  main_board_only: false,
};

const TREND_HORIZONS = [2, 3, 4, 5] as const;

function persistPredictPrefs(mode: PredictMode, horizonDays: number) {
  void saveUserSettings({
    predictMode: mode,
    ...(mode === "trend" && horizonDays >= 2
      ? { predictHorizonDays: horizonDays }
      : {}),
  });
}

function persistLookback(days: number) {
  void saveUserSettings({ lookbackDays: days });
}

function persistScreenCompose(compose: StrategyCompose) {
  void saveUserSettings({ screenCompose: compose });
}

function hydrateMonitor(snap: UserDataSnapshot | null | undefined) {
  if (!snap) return;
  useMonitorStore.setState({
    rules: snap.monitorRules ?? [],
    alerts: (snap.monitorAlerts ?? []).slice(0, 50),
    wantEnabled: snap.settings?.monitorEnabled === true,
  });
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

function upsertStrategyCompose(
  code: string,
  compose: StrategyCompose,
  memoryMap: Record<string, StrategyCompose>,
): Record<string, StrategyCompose> {
  const key = composeKey(code);
  const normalized: Record<string, StrategyCompose> = {};
  for (const [k, v] of Object.entries({ ...memoryMap, [key]: cloneCompose(compose) })) {
    normalized[composeKey(k)] = v;
  }
  void persistStrategyMap(normalized).catch((e) =>
    console.warn("保存信号组合失败", e),
  );
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
  /** 形态图周期（与预测回看解耦） */
  klinePeriod: KlinePeriod;
  /** MACD 金叉/死叉主图标记 */
  bsMarkers: BsMarker[];
  backtest: BacktestResult | null;
  watchlist: Stock[];
  poolGroups: PoolGroup[];
  poolItems: PoolItem[];
  holdings: Holding[];
  journalEntries: JournalEntry[];
  loading: boolean;
  predicting: boolean;
  loadingKlines: boolean;
  loadingBacktest: boolean;
  error: string;
  analysisSeq: number;
  chartSeq: number;

  /** 预测页组合面板 UI */
  composePanelOpen: boolean;
  composePanelHeight: number;

  init: () => Promise<void>;
  /** 设置页导入用户数据后刷新内存 */
  applyUserSnapshot: (snap: UserDataSnapshot) => void;
  selectStock: (stock: Stock) => void;
  setAlgorithm: (id: string) => void;
  setLookbackDays: (days: number) => void;
  setKlinePeriod: (period: KlinePeriod) => void;
  loadChartKlines: () => Promise<void>;
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
  persistPoolState: (groups: PoolGroup[], items: PoolItem[]) => void;
  addToPool: (stock: Stock, groupId: string) => void;
  removeFromPool: (code: string, groupId: string) => void;
  movePoolItem: (code: string, fromGroupId: string, toGroupId: string) => void;
  upsertPoolGroup: (group: PoolGroup) => void;
  removePoolGroup: (id: string) => void;
  setHoldings: (next: Holding[]) => void;
  upsertHolding: (holding: Holding) => void;
  removeHolding: (code: string) => void;
  addJournalEntry: (entry: JournalEntry) => void;
  updateJournalEntry: (entry: JournalEntry) => void;
  removeJournalEntry: (id: string) => void;
  setJournalEntries: (entries: JournalEntry[]) => void;
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
  setComposePanelOpen: (open: boolean) => void;
  setComposePanelHeight: (h: number) => void;
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
  predictMode: "daily",
  horizonDays: 1,
  prediction: null,
  klines: [],
  klinePeriod: "day",
  bsMarkers: [],
  backtest: null,
  watchlist: [],
  poolGroups: DEFAULT_POOL_GROUPS,
  poolItems: [],
  holdings: [],
  journalEntries: [],
  composePanelOpen: true,
  composePanelHeight: 232,
  loading: false,
  predicting: false,
  loadingKlines: false,
  loadingBacktest: false,
  error: "",
  analysisSeq: 0,
  chartSeq: 0,

  screenUniverse: "mixed",
  screenFilters: { ...DEFAULT_SCREEN_FILTERS },
  screenCompose: null,
  screenHorizonDays: 1,
  screenTopN: 20,
  screenResult: null,
  screening: false,
  screenProgress: { done: 0, total: 0, code: "" },

  init: async () => {
    set({ loading: true, error: "" });
    try {
      let userSnap: UserDataSnapshot | null = null;
      try {
        userSnap = await bootstrapUserPersistence();
      } catch (e) {
        console.warn("[stock-predict] user persistence bootstrap failed:", e);
      }

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

      const known = [...stocks, ...hotStocksFromApi];
      const poolGroups =
        userSnap?.poolGroups && userSnap.poolGroups.length > 0
          ? userSnap.poolGroups
          : DEFAULT_POOL_GROUPS;
      const poolItems = userSnap?.poolItems ?? [];
      const holdings = userSnap?.holdings ?? [];
      const journalEntries = userSnap?.journalEntries ?? [];

      const watchFromPool = watchlistFromPool(poolItems);
      let watchlist =
        watchFromPool.length > 0
          ? watchFromPool.map((s) => enrichStock(s, known))
          : (userSnap?.watchlist ?? []).map((s) => enrichStock(s, known));
      watchlist = await enrichSparseWatchlist(watchlist);
      const strategyMap = normalizeStrategyMap(userSnap?.strategyMap ?? {});

      const settings = userSnap?.settings ?? {};
      let predictMode: PredictMode =
        settings.predictMode === "trend" || settings.predictMode === "daily"
          ? settings.predictMode
          : "daily";
      let horizonDays = 1;
      if (predictMode === "trend") {
        const n = settings.predictHorizonDays ?? 3;
        horizonDays = TREND_HORIZONS.includes(n as (typeof TREND_HORIZONS)[number])
          ? n
          : 3;
      }

      const savedLookback = settings.lookbackDays;
      let lookbackDays =
        savedLookback && [25, 50, 60, 90, 120].includes(savedLookback)
          ? savedLookback
          : 50;

      const selectedStock =
        (hotStocksFromApi[0] ?? stocks.find((s) => s.is_hot) ?? stocks[0]) ?? null;
      if (selectedStock) {
        const saved = strategyMap[composeKey(selectedStock.code)];
        if (saved?.lookback_days) lookbackDays = saved.lookback_days;
      }

      const savedScreen = settings.screenCompose;
      const screenCompose = savedScreen
        ? mergeWithDefault(savedScreen, screenDefault, 50)
        : screenDefault
          ? cloneCompose(screenDefault)
          : null;
      if (screenCompose) persistScreenCompose(screenCompose);

      hydrateMonitor(userSnap);

      set({
        stocks,
        hotStocks: hotStocksFromApi.length > 0 ? hotStocksFromApi : stocks.filter((s) => s.is_hot),
        algorithms,
        strategySources,
        defaultCompose,
        strategyMap,
        selectedStock,
        watchlist,
        poolGroups,
        poolItems,
        holdings,
        journalEntries,
        lookbackDays,
        predictMode,
        horizonDays,
        screenCompose,
        composePanelOpen: settings.predictComposeCollapsed === true ? false : true,
        composePanelHeight:
          typeof settings.predictComposeHeight === "number" &&
          settings.predictComposeHeight >= 116 &&
          settings.predictComposeHeight <= 464
            ? settings.predictComposeHeight
            : 232,
        loading: false,
        error: softErrors.length > 0 ? softErrors.join("；") : "",
      });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  applyUserSnapshot: (snap) => {
    const strategyMap = normalizeStrategyMap(snap.strategyMap ?? {});
    const settings = snap.settings ?? {};
    const predictMode: PredictMode =
      settings.predictMode === "trend" || settings.predictMode === "daily"
        ? settings.predictMode
        : get().predictMode;
    let horizonDays = get().horizonDays;
    if (predictMode === "trend") {
      const n = settings.predictHorizonDays ?? horizonDays;
      horizonDays = TREND_HORIZONS.includes(n as (typeof TREND_HORIZONS)[number])
        ? n
        : 3;
    } else {
      horizonDays = 1;
    }
    const lookback =
      settings.lookbackDays && [25, 50, 60, 90, 120].includes(settings.lookbackDays)
        ? settings.lookbackDays
        : get().lookbackDays;

    const poolGroups =
      snap.poolGroups && snap.poolGroups.length > 0
        ? snap.poolGroups
        : DEFAULT_POOL_GROUPS;
    const poolItems = snap.poolItems ?? [];
    const holdings = snap.holdings ?? [];
    const journalEntries = snap.journalEntries ?? [];
    const watchFromPool = watchlistFromPool(poolItems);
    const watchlist =
      watchFromPool.length > 0 ? watchFromPool : (snap.watchlist ?? []);

    hydrateMonitor(snap);
    set({
      watchlist,
      poolGroups,
      poolItems,
      holdings,
      journalEntries,
      strategyMap,
      lookbackDays: lookback,
      predictMode,
      horizonDays,
      screenCompose: settings.screenCompose
        ? cloneCompose(settings.screenCompose)
        : get().screenCompose,
      composePanelOpen: settings.predictComposeCollapsed === true ? false : true,
      composePanelHeight:
        typeof settings.predictComposeHeight === "number"
          ? settings.predictComposeHeight
          : get().composePanelHeight,
    });
  },

  selectStock: (stock) => {
    const compose = get().getComposeForStock(stock.code);
    set({
      selectedStock: stock,
      prediction: null,
      klines: [],
      klinePeriod: "day",
      bsMarkers: [],
      backtest: null,
      lookbackDays: compose.lookback_days,
    });
    void get().runPrediction();
  },

  setKlinePeriod: (period) => {
    if (get().klinePeriod === period) return;
    set({ klinePeriod: period });
    void get().loadChartKlines();
  },

  loadChartKlines: async () => {
    const { selectedStock, klinePeriod, lookbackDays } = get();
    if (!selectedStock) return;

    const seq = get().chartSeq + 1;
    set({ chartSeq: seq, loadingKlines: true });

    try {
      const limit = chartBarLimit(klinePeriod, lookbackDays);
      const bars = await getStockKlines(selectedStock, limit, klinePeriod);
      if (get().chartSeq !== seq) return;
      set({ klines: bars, loadingKlines: false });
    } catch (err) {
      if (get().chartSeq !== seq) return;
      set({
        error: `加载 K 线失败: ${String(err)}`,
        loadingKlines: false,
        klines: [],
      });
    }
  },

  setAlgorithm: (id) => {
    set({ activeAlgorithm: id });
    if (get().selectedStock) void get().runPrediction();
  },

  setLookbackDays: (days) => {
    persistLookback(days);
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
        prevHorizon >= 2 && prevHorizon <= 5 ? prevHorizon : 3;
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
    const saved = strategyMap[key];
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
        persistLookback(tuned.lookback_days);
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
        bsMarkers: result.bs_markers ?? [],
        backtest: result.backtest,
        predicting: false,
        loadingBacktest: false,
      });

      // 日 K 与预测窗口对齐，直接用分析结果；其它周期独立拉取
      if (get().klinePeriod === "day") {
        set({ klines: result.klines, loadingKlines: false });
      } else {
        void get().loadChartKlines();
      }
    } catch (err) {
      if (get().analysisSeq !== seq) return;
      set({
        error: String(err),
        predicting: false,
        loadingKlines: false,
        loadingBacktest: false,
        bsMarkers: [],
      });
    }
  },

  loadStockAnalysis: async () => {
    await get().runPrediction();
  },

  toggleWatchlist: (stock) => {
    const items = get().poolItems;
    const inWatch = items.some(
      (i) => i.groupId === GROUP_WATCH && i.code === stock.code,
    );
    const nextItems = inWatch
      ? items.filter(
          (i) => !(i.groupId === GROUP_WATCH && i.code === stock.code),
        )
      : [...items, poolItemFromStock(stock, GROUP_WATCH, items.length)];
    get().persistPoolState(get().poolGroups, nextItems);
  },

  persistPoolState: (groups, items) => {
    const holdings = get().holdings;
    const withoutMirror = items.filter(
      (i) => i.groupId !== GROUP_HOLDINGS_MIRROR,
    );
    const withMirror = [...withoutMirror, ...holdingsMirrorItems(holdings)];
    void savePool(groups, withMirror).catch((e) =>
      console.warn("保存股票池失败", e),
    );
    set({
      poolGroups: groups,
      poolItems: withMirror,
      watchlist: watchlistFromPool(withMirror),
    });
  },

  addToPool: (stock, groupId) => {
    const items = get().poolItems;
    const idx = items.findIndex(
      (i) => i.groupId === groupId && i.code === stock.code,
    );
    let nextItems: PoolItem[];
    if (idx >= 0) {
      const prev = items[idx];
      nextItems = items.map((i, n) =>
        n === idx ? poolItemFromStock(stock, groupId, prev.sortOrder) : i,
      );
    } else {
      const sortOrder = items.filter((i) => i.groupId === groupId).length;
      nextItems = [...items, poolItemFromStock(stock, groupId, sortOrder)];
    }
    get().persistPoolState(get().poolGroups, nextItems);
  },

  removeFromPool: (code, groupId) => {
    const nextItems = get().poolItems.filter(
      (i) => !(i.groupId === groupId && i.code === code),
    );
    get().persistPoolState(get().poolGroups, nextItems);
  },

  movePoolItem: (code, fromGroupId, toGroupId) => {
    if (fromGroupId === toGroupId) return;
    const items = get().poolItems;
    const item = items.find(
      (i) => i.groupId === fromGroupId && i.code === code,
    );
    if (!item) return;
    const without = items.filter(
      (i) => !(i.groupId === fromGroupId && i.code === code),
    );
    const existsInTo = without.some(
      (i) => i.groupId === toGroupId && i.code === code,
    );
    const nextItems = existsInTo
      ? without
      : [
          ...without,
          {
            ...item,
            groupId: toGroupId,
            sortOrder: without.filter((i) => i.groupId === toGroupId).length,
          },
        ];
    get().persistPoolState(get().poolGroups, nextItems);
  },

  upsertPoolGroup: (group) => {
    const groups = get().poolGroups;
    const idx = groups.findIndex((g) => g.id === group.id);
    const nextGroups =
      idx >= 0
        ? groups.map((g, i) => (i === idx ? group : g))
        : [...groups, group];
    get().persistPoolState(nextGroups, get().poolItems);
  },

  removePoolGroup: (id) => {
    if (FIXED_POOL_GROUP_IDS.has(id)) return;
    const nextGroups = get().poolGroups.filter((g) => g.id !== id);
    const nextItems = get().poolItems.map((i) =>
      i.groupId === id ? { ...i, groupId: GROUP_REMOVED } : i,
    );
    get().persistPoolState(nextGroups, nextItems);
  },

  setHoldings: (next) => {
    void saveHoldings(next).catch((e) => console.warn("保存持仓失败", e));
    const { poolGroups, poolItems } = get();
    const withoutMirror = poolItems.filter(
      (i) => i.groupId !== GROUP_HOLDINGS_MIRROR,
    );
    const withMirror = [...withoutMirror, ...holdingsMirrorItems(next)];
    void savePool(poolGroups, withMirror).catch((e) =>
      console.warn("保存股票池失败", e),
    );
    set({
      holdings: next,
      poolItems: withMirror,
      watchlist: watchlistFromPool(withMirror),
    });
  },

  upsertHolding: (holding) => {
    const holdings = get().holdings;
    const idx = holdings.findIndex((h) => h.code === holding.code);
    const next =
      idx >= 0
        ? holdings.map((h, i) => (i === idx ? holding : h))
        : [...holdings, holding];
    get().setHoldings(next);
  },

  removeHolding: (code) => {
    get().setHoldings(get().holdings.filter((h) => h.code !== code));
  },

  addJournalEntry: (entry) => {
    const next = [...get().journalEntries, entry];
    void saveJournalEntries(next).catch((e) =>
      console.warn("保存复盘失败", e),
    );
    set({ journalEntries: next });
  },

  updateJournalEntry: (entry) => {
    const next = get().journalEntries.map((e) =>
      e.id === entry.id ? entry : e,
    );
    void saveJournalEntries(next).catch((e) =>
      console.warn("保存复盘失败", e),
    );
    set({ journalEntries: next });
  },

  removeJournalEntry: (id) => {
    const next = get().journalEntries.filter((e) => e.id !== id);
    void saveJournalEntries(next).catch((e) =>
      console.warn("保存复盘失败", e),
    );
    set({ journalEntries: next });
  },

  setJournalEntries: (entries) => {
    void saveJournalEntries(entries).catch((e) =>
      console.warn("保存复盘失败", e),
    );
    set({ journalEntries: entries });
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
    persistScreenCompose(next);
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
      persistScreenCompose(def);
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

  setComposePanelOpen: (open) => {
    set({ composePanelOpen: open });
    void saveUserSettings({ predictComposeCollapsed: !open });
  },

  setComposePanelHeight: (h) => {
    set({ composePanelHeight: h });
    void saveUserSettings({ predictComposeHeight: h });
  },
}));

function normalizeStrategyMap(
  map: Record<string, StrategyCompose>,
): Record<string, StrategyCompose> {
  const out: Record<string, StrategyCompose> = {};
  for (const [k, v] of Object.entries(map)) {
    if (!isValidCompose(v)) continue;
    out[composeKey(k)] = cloneCompose(v);
  }
  return out;
}

function enrichStock(s: Stock, known: Stock[]): Stock {
  const hit = known.find((k) => k.code === s.code);
  if (!hit) return s;
  return {
    ...s,
    name: s.name && s.name !== s.code ? s.name : hit.name,
    market: s.market || hit.market,
    sector: s.sector || hit.sector,
    price: s.price ?? hit.price,
    change_pct: s.change_pct ?? hit.change_pct,
  };
}

/** 导入时仅有代码/名称占位的自选，尝试搜索补全。 */
async function enrichSparseWatchlist(current: Stock[]): Promise<Stock[]> {
  const sparse = current.filter(
    (s) => !s.name || s.name === s.code || !s.market,
  );
  if (sparse.length === 0) return current;

  const recovered: Stock[] = [];
  for (const s of sparse) {
    try {
      const results = await searchStocks(s.code, 8);
      const match = results.find((r) => r.code === s.code) ?? results[0];
      if (match) recovered.push(match);
    } catch {
      /* ignore */
    }
  }
  if (recovered.length === 0) return current;

  const merged = current.map((s) => {
    const r = recovered.find((x) => x.code === s.code);
    return r ? { ...s, ...r } : s;
  });
  void persistWatchlist(merged).catch(() => undefined);
  return merged;
}
