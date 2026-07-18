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
        <div className="flex h-7 w-7 items-center justify-center overflow-hidden rounded-md bg-gradient-to-br from-indigo-950 via-emerald-800 to-amber-700/80 shadow-sm shadow-emerald-500/25 ring-1 ring-white/10">
          <svg viewBox="0 0 32 32" className="h-4 w-4" aria-hidden>
            <defs>
              <radialGradient id="orb" cx="40%" cy="35%" r="65%">
                <stop offset="0%" stopColor="#ecfdf5" />
                <stop offset="55%" stopColor="#34d399" />
                <stop offset="100%" stopColor="#312e81" />
              </radialGradient>
            </defs>
            <circle cx="16" cy="16" r="10" fill="url(#orb)" opacity="0.95" />
            <path
              d="M10 20 L14 14 L18 17 L22 10"
              fill="none"
              stroke="#fef3c7"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-semibold tracking-wide">以太测</div>
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
