import { invoke } from "@tauri-apps/api/core";
import type { AlgorithmInfo, PredictionResult, Stock, StocksPayload } from "@/types";

export async function loadStocks() {
  return invoke<StocksPayload>("load_stocks");
}

export async function searchStocks(query: string, limit?: number) {
  return invoke<Stock[]>("search_stocks", { query, limit });
}

export async function predictStock(stock: Stock, algorithm?: string) {
  return invoke<PredictionResult>("predict_stock", { stock, algorithm });
}

export async function listAlgorithms() {
  return invoke<AlgorithmInfo[]>("list_algorithms");
}
