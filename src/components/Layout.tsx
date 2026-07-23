import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import { useMonitorStore } from "@/stores/monitorStore";

const links = [
  { to: "/", label: "行情", icon: "⌂", end: true },
  { to: "/screen", label: "选股", icon: "◎" },
  { to: "/pool", label: "股票池", icon: "★" },
  { to: "/holdings", label: "持仓", icon: "▣" },
  { to: "/review", label: "复盘", icon: "◐" },
];

function useWideNav() {
  const [wide, setWide] = useState(
    () => typeof window !== "undefined" && window.innerWidth >= 900,
  );
  useEffect(() => {
    const onResize = () => setWide(window.innerWidth >= 900);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return wide;
}

export function Layout() {
  const init = useStockStore((s) => s.init);
  const selectedStock = useStockStore((s) => s.selectedStock);
  const loading = useStockStore((s) => s.loading);
  const ensureListeners = useMonitorStore((s) => s.ensureListeners);
  const monitorRunning = useMonitorStore((s) => s.running);
  const navigate = useNavigate();
  const location = useLocation();
  const wide = useWideNav();

  const immersive = useMemo(() => {
    const sp = new URLSearchParams(location.search);
    return sp.get("immersive") === "1" || location.pathname.startsWith("/compare");
  }, [location.pathname, location.search]);

  useEffect(() => {
    void init().then(() => {
      void useStockStore.getState().runPrediction();
      void ensureListeners();
    });
  }, [init, ensureListeners]);

  const hideChrome = immersive;

  return (
    <div
      className={cn(
        "relative flex h-dvh overflow-hidden bg-[#030712] text-slate-100",
        wide && !hideChrome ? "flex-row" : "flex-col",
      )}
    >
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute -left-24 top-0 h-72 w-72 rounded-full bg-emerald-500/10 blur-3xl animate-pulse-glow" />
        <div className="absolute -right-24 bottom-20 h-72 w-72 rounded-full bg-cyan-500/10 blur-3xl animate-pulse-glow" />
      </div>

      {wide && !hideChrome && (
        <aside className="relative z-20 flex w-44 shrink-0 flex-col border-r border-white/5 bg-slate-950/90 pt-[env(safe-area-inset-top)] backdrop-blur-xl">
          <div className="flex items-center gap-2 px-3 py-3">
            <BrandMark />
            <span className="text-sm font-semibold">以太测</span>
          </div>
          <nav className="flex flex-1 flex-col gap-0.5 px-2 py-2">
            {links.map((link) => (
              <NavLink
                key={link.to}
                to={link.to}
                end={link.end}
                className={({ isActive }) =>
                  cn(
                    "flex items-center gap-2 rounded-lg px-2.5 py-2 text-xs transition",
                    isActive
                      ? "bg-emerald-500/15 text-emerald-300"
                      : "text-slate-500 hover:bg-white/5 hover:text-slate-300",
                  )
                }
              >
                <span className="opacity-80">{link.icon}</span>
                {link.label}
              </NavLink>
            ))}
          </nav>
          <button
            type="button"
            onClick={() => navigate("/settings")}
            className="m-2 rounded-lg border border-white/5 px-2.5 py-2 text-left text-xs text-slate-400 hover:bg-white/5"
          >
            ⚙ 设置 / 备份
          </button>
        </aside>
      )}

      <div className="relative z-10 flex min-h-0 min-w-0 flex-1 flex-col">
        {!hideChrome && !wide && (
          <header className="relative z-10 flex h-11 shrink-0 items-center gap-2.5 border-b border-white/5 bg-slate-950/80 px-3 pt-[env(safe-area-inset-top)] backdrop-blur-xl">
            <BrandMark />
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold tracking-wide">以太测</div>
            </div>
            <div className="truncate text-[11px] text-slate-500">
              {loading
                ? "加载中..."
                : monitorRunning
                  ? "盯盘中"
                  : selectedStock
                    ? `${selectedStock.name}`
                    : "智能涨跌预测"}
            </div>
            <button
              type="button"
              onClick={() => navigate("/settings")}
              className="shrink-0 rounded-lg border border-white/10 px-2 py-1 text-sm text-slate-400"
              aria-label="设置"
            >
              ⚙
            </button>
          </header>
        )}

        <main className="relative z-10 flex min-h-0 flex-1 flex-col overflow-hidden">
          <Outlet />
        </main>

        {!hideChrome && !wide && (
          <nav className="relative z-20 grid h-12 shrink-0 grid-cols-5 border-t border-white/5 bg-slate-950/95 px-1 pb-[env(safe-area-inset-bottom)] backdrop-blur-xl">
            {links.map((link) => (
              <NavLink
                key={link.to}
                to={link.to}
                end={link.end}
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
        )}
      </div>
    </div>
  );
}

function BrandMark() {
  return (
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
  );
}
