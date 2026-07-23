import { Navigate, Route, Routes, useParams } from "react-router-dom";
import { Layout } from "@/components/Layout";
import { HomePage } from "@/pages/HomePage";
import { PredictPage } from "@/pages/PredictPage";
import { ScreenPage } from "@/pages/ScreenPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { PoolPage } from "@/pages/PoolPage";
import { HoldingsPage } from "@/pages/HoldingsPage";
import { ReviewPage } from "@/pages/ReviewPage";
import { StockWorkbenchPage } from "@/pages/StockWorkbenchPage";
import { ComparePage } from "@/pages/ComparePage";
import { useStockStore } from "@/stores/stockStore";
import { useEffect } from "react";

function PredictRedirect() {
  const selected = useStockStore((s) => s.selectedStock);
  if (selected?.code) {
    return <Navigate to={`/stock/${selected.code}`} replace />;
  }
  return <Navigate to="/" replace />;
}

function StockRoute() {
  const { code } = useParams();
  const stocks = useStockStore((s) => s.stocks);
  const hotStocks = useStockStore((s) => s.hotStocks);
  const watchlist = useStockStore((s) => s.watchlist);
  const poolItems = useStockStore((s) => s.poolItems);
  const selected = useStockStore((s) => s.selectedStock);
  const selectStock = useStockStore((s) => s.selectStock);

  useEffect(() => {
    if (!code) return;
    if (selected?.code === code) return;
    const fromPool = poolItems.find((i) => i.code === code);
    const found =
      watchlist.find((s) => s.code === code) ||
      hotStocks.find((s) => s.code === code) ||
      stocks.find((s) => s.code === code) ||
      (fromPool
        ? {
            code: fromPool.code,
            name: fromPool.name,
            market: fromPool.market,
            sector: fromPool.sector,
          }
        : null);
    if (found) selectStock(found);
  }, [code, selected?.code, stocks, hotStocks, watchlist, poolItems, selectStock]);

  return <StockWorkbenchPage />;
}

export default function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<HomePage />} />
        <Route path="market" element={<Navigate to="/" replace />} />
        <Route path="screen" element={<ScreenPage />} />
        <Route path="pool" element={<PoolPage />} />
        <Route path="holdings" element={<HoldingsPage />} />
        <Route path="review" element={<ReviewPage />} />
        <Route path="compare" element={<ComparePage />} />
        <Route path="stock/:code" element={<StockRoute />} />
        <Route path="predict" element={<PredictRedirect />} />
        <Route path="watchlist" element={<Navigate to="/pool" replace />} />
        <Route path="settings" element={<SettingsPage />} />
        {/* legacy predict page kept for deep links during transition */}
        <Route path="predict/legacy" element={<PredictPage />} />
      </Route>
    </Routes>
  );
}
