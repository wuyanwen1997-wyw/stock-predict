import { invoke } from "@tauri-apps/api/core";
import type {
  AlgorithmInfo,
  AnalysisResult,
  BacktestResult,
  DailyBar,
  PredictionResult,
  Stock,
  StocksPayload,
  StrategyCompose,
  StrategySourceInfo,
} from "@/types";

export async function loadStocks() {
  return invoke<StocksPayload>("load_stocks");
}

export async function searchStocks(query: string, limit?: number) {
  return invoke<Stock[]>("search_stocks", { query, limit });
}

export async function analyzeStock(
  stock: Stock,
  algorithm?: string,
  lookbackDays?: number,
  compose?: StrategyCompose,
) {
  return invoke<AnalysisResult>("analyze_stock", {
    stock,
    algorithm,
    lookbackDays,
    compose,
  });
}

export async function predictStock(
  stock: Stock,
  algorithm?: string,
  lookbackDays?: number,
  compose?: StrategyCompose,
) {
  return invoke<PredictionResult>("predict_stock", {
    stock,
    algorithm,
    lookbackDays,
    compose,
  });
}

export async function getStockKlines(stock: Stock, limit?: number) {
  return invoke<DailyBar[]>("get_stock_klines", { stock, limit });
}

export async function backtestStock(
  stock: Stock,
  algorithm?: string,
  days?: number,
  compose?: StrategyCompose,
) {
  return invoke<BacktestResult>("backtest_stock", { stock, algorithm, days, compose });
}

export async function listAlgorithms() {
  return invoke<AlgorithmInfo[]>("list_algorithms");
}

export async function listStrategySources() {
  return invoke<StrategySourceInfo[]>("list_strategy_sources");
}

export async function defaultStrategyCompose() {
  return invoke<StrategyCompose>("default_strategy_compose");
}
