import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";

export function SettingsPage() {
  const algorithms = useStockStore((s) => s.algorithms);
  const activeAlgorithm = useStockStore((s) => s.activeAlgorithm);
  const setAlgorithm = useStockStore((s) => s.setAlgorithm);

  return (
    <div className="p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -12 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-8"
      >
        <h1 className="text-2xl font-semibold text-slate-100">设置</h1>
        <p className="mt-2 text-sm text-slate-400">
          选择预测算法。当前版本仅启用占位模型，其余算法已预留接口。
        </p>
      </motion.header>

      <div className="space-y-3">
        {algorithms.map((algo, i) => (
          <motion.button
            key={algo.id}
            initial={{ opacity: 0, x: -12 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: i * 0.05 }}
            type="button"
            disabled={!algo.enabled}
            onClick={() => setAlgorithm(algo.id)}
            className={cn(
              "w-full rounded-2xl border p-5 text-left transition-all duration-200",
              activeAlgorithm === algo.id
                ? "border-emerald-500/40 bg-emerald-500/10"
                : "border-white/5 bg-slate-900/50 hover:border-white/10",
              !algo.enabled && "cursor-not-allowed opacity-50",
            )}
          >
            <div className="flex items-center justify-between">
              <span className="font-medium text-slate-200">{algo.name}</span>
              <span
                className={cn(
                  "rounded-full px-2 py-0.5 text-xs",
                  algo.enabled
                    ? "bg-emerald-500/15 text-emerald-300"
                    : "bg-slate-700 text-slate-400",
                )}
              >
                {algo.enabled ? "可用" : "预留"}
              </span>
            </div>
            <p className="mt-2 text-sm leading-relaxed text-slate-500">
              {algo.description}
            </p>
          </motion.button>
        ))}
      </div>

      <div className="mt-8 rounded-2xl border border-white/5 bg-slate-900/30 p-5 text-sm text-slate-500">
        <p className="font-medium text-slate-400">扩展说明</p>
        <ul className="mt-3 list-inside list-disc space-y-1.5 leading-relaxed">
          <li>后端算法入口：<code className="text-slate-400">src-tauri/src/predictor.rs</code></li>
          <li>新增算法后在 <code className="text-slate-400">commands.rs</code> 的 list_algorithms 中注册</li>
          <li>股票列表：<code className="text-slate-400">src-tauri/resources/stocks.json</code></li>
        </ul>
      </div>
    </div>
  );
}
