import { NavLink, Outlet } from "react-router-dom";
import { useEffect } from "react";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

const links = [
  { to: "/", label: "首页", icon: "⌂" },
  { to: "/predict", label: "预测", icon: "◈" },
  { to: "/watchlist", label: "自选", icon: "★" },
  { to: "/settings", label: "设置", icon: "⚙" },
];

export function Layout() {
  const init = useStockStore((s) => s.init);
  const selectedStock = useStockStore((s) => s.selectedStock);
  const loading = useStockStore((s) => s.loading);

  useEffect(() => {
    void init().then(() => {
      void useStockStore.getState().runPrediction();
    });
  }, [init]);

  return (
    <div className="relative flex h-dvh flex-col overflow-hidden bg-[#030712] text-slate-100">
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute -left-24 top-0 h-72 w-72 rounded-full bg-emerald-500/10 blur-3xl animate-pulse-glow" />
        <div className="absolute -right-24 bottom-20 h-72 w-72 rounded-full bg-cyan-500/10 blur-3xl animate-pulse-glow" />
      </div>

      {/* Compact top brand strip */}
      <header className="relative z-10 flex h-11 shrink-0 items-center gap-2.5 border-b border-white/5 bg-slate-950/80 px-3 backdrop-blur-xl">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-gradient-to-br from-emerald-400 to-cyan-500 text-xs font-bold text-slate-950">
          S
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-semibold tracking-wide">StockPredict</div>
        </div>
        <div className="truncate text-[11px] text-slate-500">
          {loading
            ? "加载中..."
            : selectedStock
              ? `${selectedStock.name}`
              : "智能涨跌预测"}
        </div>
      </header>

      {/* Middle: page content (scroll handled by each page) */}
      <main className="relative z-10 flex min-h-0 flex-1 flex-col overflow-hidden">
        <Outlet />
      </main>

      {/* Compact bottom nav */}
      <nav className="relative z-20 grid h-12 shrink-0 grid-cols-4 border-t border-white/5 bg-slate-950/95 px-1 pb-[env(safe-area-inset-bottom)] backdrop-blur-xl">
        {links.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            end={link.to === "/"}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center gap-0.5 rounded-lg text-[10px] transition",
                isActive
                  ? "text-emerald-300"
                  : "text-slate-500 active:text-slate-300",
              )
            }
          >
            <span className="text-sm leading-none opacity-80">{link.icon}</span>
            {link.label}
          </NavLink>
        ))}
      </nav>
    </div>
  );
}
