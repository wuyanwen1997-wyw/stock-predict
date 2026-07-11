export interface Stock {
  code: string;
  name: string;
  market: string;
  sector: string;
  price?: number;
  change_pct?: number;
  is_hot?: boolean;
}

export interface StocksPayload {
  stocks: Stock[];
  hot_stocks: Stock[];
}

export interface PricePoint {
  time: string;
  price: number;
  volume: number;
}

export interface ScenarioForecast {
  label: string;
  open_price: number;
  close_price: number;
  high_price: number;
  low_price: number;
  change_pct: number;
  path: PricePoint[];
}

export interface PredictionResult {
  stock: Stock;
  predict_date: string;
  current_price: number;
  up_probability: number;
  down_probability: number;
  flat_probability: number;
  confidence: number;
  algorithm: string;
  high_open: ScenarioForecast;
  low_open: ScenarioForecast;
  summary: string;
}

export interface AlgorithmInfo {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}
