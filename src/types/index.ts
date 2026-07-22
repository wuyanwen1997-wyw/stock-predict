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
  /** 人气榜或部分行情失败时的可读告警 */
  warning?: string;
}

export interface AnalysisResult {
  prediction: PredictionResult;
  klines: DailyBar[];
  backtest: BacktestResult;
}

export interface DailyBar {
  date: string;
  open: number;
  close: number;
  high: number;
  low: number;
  volume: number;
  change_pct?: number;
}

export interface BacktestRecord {
  date: string;
  predict_date: string;
  close_price: number;
  next_close: number;
  change_pct: number;
  predicted: string;
  actual: string;
  up_probability: number;
  down_probability: number;
  confidence: number;
  high_confidence: boolean;
  correct: boolean;
}

export interface BacktestResult {
  stock: Stock;
  algorithm: string;
  total_samples: number;
  direction_accuracy: number;
  actionable_accuracy: number;
  all_day_accuracy?: number;
  actionable_samples?: number;
  selective_mode?: boolean;
  up_hit_rate: number;
  down_hit_rate: number;
  up_hit_rate_actionable?: number;
  down_hit_rate_actionable?: number;
  up_samples?: number;
  down_samples?: number;
  up_samples_actionable?: number;
  down_samples_actionable?: number;
  high_confidence_samples: number;
  high_confidence_accuracy: number;
  high_confidence_threshold: number;
  flat_threshold_pct: number;
  lookback_days: number;
  /** 预测跨度：1=次日，2–5=多日累计 */
  horizon_days?: number;
  summary: string;
  records: BacktestRecord[];
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

export interface SignalContribution {
  id: string;
  name: string;
  category: string;
  up_probability: number;
  down_probability: number;
  confidence: number;
  weight: number;
  weight_normalized: number;
  note: string;
  status: string;
}

export interface StrategySourceInfo {
  id: string;
  name: string;
  category: string;
  description: string;
  backtestable: boolean;
  available: boolean;
}

export interface StrategySourceConfig {
  id: string;
  enabled: boolean;
  weight: number;
}

export interface StrategyCompose {
  sources: StrategySourceConfig[];
  lookback_days: number;
}

export interface PredictionResult {
  stock: Stock;
  predict_date: string;
  current_price: number;
  up_probability: number;
  down_probability: number;
  flat_probability: number;
  confidence: number;
  predicted: string;
  high_confidence: boolean;
  high_confidence_threshold: number;
  algorithm: string;
  high_open: ScenarioForecast;
  low_open: ScenarioForecast;
  summary: string;
  signals?: SignalContribution[];
  /** 预测跨度：1=次日，2–5=多日累计 */
  horizon_days?: number;
}

export interface AlgorithmInfo {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}

export type ScreenUniverse = "hot" | "watchlist" | "seed" | "mixed";

export interface ScreenFilters {
  exclude_st: boolean;
  min_price?: number | null;
  min_change_pct?: number | null;
  max_change_pct?: number | null;
  main_board_only: boolean;
}

export interface ScreenRequest {
  universe: ScreenUniverse;
  watchlist: Stock[];
  filters: ScreenFilters;
  compose?: StrategyCompose;
  horizon_days: number;
  lookback_days: number;
  top_n: number;
  concurrency: number;
}

export interface ScreenHit {
  stock: Stock;
  up_probability: number;
  down_probability: number;
  confidence: number;
  direction: string;
  factor_score: number;
  hints: string[];
  error?: string;
}

export interface ScreenProgressEvent {
  done: number;
  total: number;
  code: string;
}

export interface ScreenResult {
  hits: ScreenHit[];
  universe_size: number;
  filtered_size: number;
  scored_size: number;
  failed_size: number;
  elapsed_ms: number;
  summary: string;
  timed_out?: boolean;
}
