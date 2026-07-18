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
  horizonDays?: number,
) {
  return invoke<AnalysisResult>("analyze_stock", {
    stock,
    algorithm,
    lookbackDays,
    compose,
    horizonDays,
  });
}

export async function predictStock(
  stock: Stock,
  algorithm?: string,
  lookbackDays?: number,
  compose?: StrategyCompose,
  horizonDays?: number,
) {
  return invoke<PredictionResult>("predict_stock", {
    stock,
    algorithm,
    lookbackDays,
    compose,
    horizonDays,
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
  horizonDays?: number,
) {
  return invoke<BacktestResult>("backtest_stock", {
    stock,
    algorithm,
    days,
    compose,
    horizonDays,
  });
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

export async function defaultStrategyComposeForStock(stock: Stock) {
  return invoke<StrategyCompose>("default_strategy_compose_for_stock", { stock });
}

export type TushareTokenStatus = {
  configured: boolean;
  hint: string;
};

export async function getTushareTokenStatus() {
  return invoke<TushareTokenStatus>("get_tushare_token_status");
}

export async function setTushareToken(token: string) {
  return invoke<TushareTokenStatus>("set_tushare_token", { token });
}
