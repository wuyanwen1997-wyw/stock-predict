import { NavLink, Outlet } from "react-router-dom";
import { useEffect } from "react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

const links = [
  { to: "/", label: "预测", icon: "◈" },
  { to: "/watchlist", label: "自选", icon: "★" },
  { to: "/settings", label: "设置", icon: "⚙" },
];

export function Layout() {
  const init = useStockStore((s) => s.init);
  const selectedStock = useStockStore((s) => s.selectedStock);
  const loading = useStockStore((s) => s.loading);
  const error = useStockStore((s) => s.error);

  useEffect(() => {
    void init().then(() => {
      void useStockStore.getState().runPrediction();
    });
  }, [init]);

  return (
    <div className="relative flex min-h-screen overflow-hidden bg-[#030712] text-slate-100">
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute -left-32 top-0 h-96 w-96 rounded-full bg-emerald-500/10 blur-3xl animate-pulse-glow" />
        <div className="absolute -right-32 bottom-0 h-96 w-96 rounded-full bg-cyan-500/10 blur-3xl animate-pulse-glow" />
        <div className="absolute left-1/2 top-1/3 h-64 w-64 -translate-x-1/2 rounded-full bg-violet-500/5 blur-3xl" />
      </div>

      <aside className="relative z-10 flex w-60 shrink-0 flex-col border-r border-white/5 bg-slate-950/70 backdrop-blur-xl">
        <div className="border-b border-white/5 px-5 py-6">
          <motion.div
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex items-center gap-3"
          >
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-emerald-400 to-cyan-500 text-lg font-bold text-slate-950 shadow-lg shadow-emerald-500/20">
              S
            </div>
            <div>
              <div className="text-lg font-semibold tracking-wide">StockPredict</div>
              <div className="text-xs text-slate-400">智能涨跌预测</div>
            </div>
          </motion.div>
        </div>

        <nav className="flex flex-col gap-1 p-3">
          {links.map((link) => (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.to === "/"}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-2.5 rounded-xl px-3 py-2.5 text-sm transition-all duration-200",
                  isActive
                    ? "bg-gradient-to-r from-emerald-500/20 to-cyan-500/10 text-emerald-300 shadow-inner shadow-emerald-500/10"
                    : "text-slate-400 hover:bg-white/5 hover:text-slate-200",
                )
              }
            >
              <span className="text-base opacity-70">{link.icon}</span>
              {link.label}
            </NavLink>
          ))}
        </nav>

        <div className="mt-auto border-t border-white/5 p-4 text-xs text-slate-500">
          {loading ? (
            <div className="animate-shimmer rounded-lg px-2 py-3">加载股票数据...</div>
          ) : error ? (
            <div className="text-amber-400">{error}</div>
          ) : selectedStock ? (
            <>
              <div className="text-slate-400">当前分析</div>
              <div className="mt-1 font-medium text-slate-200">
                {selectedStock.name}
              </div>
              <div className="mt-0.5 font-mono text-slate-500">
                {selectedStock.market}.{selectedStock.code}
              </div>
            </>
          ) : (
            <div>请选择股票</div>
          )}
        </div>
      </aside>

      <main className="relative z-10 flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
