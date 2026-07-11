import { create } from "zustand";
import { listAlgorithms, loadStocks, predictStock, searchStocks } from "@/services/api";
import type { AlgorithmInfo, PredictionResult, Stock } from "@/types";

interface StockState {
  stocks: Stock[];
  hotStocks: Stock[];
  searchResults: Stock[];
  searchQuery: string;
  searching: boolean;
  algorithms: AlgorithmInfo[];
  selectedStock: Stock | null;
  activeAlgorithm: string;
  prediction: PredictionResult | null;
  watchlist: string[];
  loading: boolean;
  predicting: boolean;
  error: string;

  init: () => Promise<void>;
  selectStock: (stock: Stock) => void;
  setAlgorithm: (id: string) => void;
  runPrediction: () => Promise<void>;
  toggleWatchlist: (code: string) => void;
  setSearchQuery: (query: string) => void;
  runSearch: () => Promise<void>;
  clearSearch: () => void;
}

export const useStockStore = create<StockState>((set, get) => ({
  stocks: [],
  hotStocks: [],
  searchResults: [],
  searchQuery: "",
  searching: false,
  algorithms: [],
  selectedStock: null,
  activeAlgorithm: "placeholder_v1",
  prediction: null,
  watchlist: [],
  loading: false,
  predicting: false,
  error: "",

  init: async () => {
    set({ loading: true, error: "" });
    try {
      const [{ stocks, hot_stocks: hotStocks }, algorithms] = await Promise.all([
        loadStocks(),
        listAlgorithms(),
      ]);
      const saved = localStorage.getItem("watchlist");
      const watchlist = saved ? (JSON.parse(saved) as string[]) : [];
      const hotStocksFromApi = hotStocks;
      set({
        stocks,
        hotStocks: hotStocksFromApi.length > 0 ? hotStocksFromApi : stocks.filter((s) => s.is_hot),
        algorithms,
        selectedStock: (hotStocksFromApi[0] ?? stocks.find((s) => s.is_hot) ?? stocks[0]) ?? null,
        watchlist,
        loading: false,
      });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  selectStock: (stock) => {
    set({ selectedStock: stock, prediction: null });
    void get().runPrediction();
  },

  setAlgorithm: (id) => {
    set({ activeAlgorithm: id });
    if (get().selectedStock) void get().runPrediction();
  },

  runPrediction: async () => {
    const { selectedStock, activeAlgorithm } = get();
    if (!selectedStock) return;

    set({ predicting: true, error: "" });
    try {
      const prediction = await predictStock(selectedStock, activeAlgorithm);
      set({ prediction, predicting: false });
    } catch (err) {
      set({ error: String(err), predicting: false });
    }
  },

  toggleWatchlist: (code) => {
    const next = get().watchlist.includes(code)
      ? get().watchlist.filter((c) => c !== code)
      : [...get().watchlist, code];
    localStorage.setItem("watchlist", JSON.stringify(next));
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
