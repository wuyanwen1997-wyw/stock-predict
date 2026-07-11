import { Route, Routes } from "react-router-dom";
import { Layout } from "@/components/Layout";
import { PredictPage } from "@/pages/PredictPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { WatchlistPage } from "@/pages/WatchlistPage";

export default function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<PredictPage />} />
        <Route path="watchlist" element={<WatchlistPage />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
