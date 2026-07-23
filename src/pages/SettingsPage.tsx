import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { useStockStore } from "@/stores/stockStore";
import {
  getTushareTokenStatus,
  setTushareToken,
  type TushareTokenStatus,
} from "@/services/api";
import {
  applyImportedSnapshotFlag,
  exportUserData,
  importUserData,
} from "@/services/userPersistence";

export function SettingsPage() {
  const algorithms = useStockStore((s) => s.algorithms);
  const activeAlgorithm = useStockStore((s) => s.activeAlgorithm);
  const setAlgorithm = useStockStore((s) => s.setAlgorithm);
  const lookbackDays = useStockStore((s) => s.lookbackDays);
  const setLookbackDays = useStockStore((s) => s.setLookbackDays);
  const applyUserSnapshot = useStockStore((s) => s.applyUserSnapshot);

  const lookbackOptions = [25, 50, 60, 90, 120];
  const [tokenInput, setTokenInput] = useState("");
  const [tokenStatus, setTokenStatus] = useState<TushareTokenStatus | null>(null);
  const [tokenMsg, setTokenMsg] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [backupMsg, setBackupMsg] = useState<string | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);

  useEffect(() => {
    getTushareTokenStatus()
      .then(setTokenStatus)
      .catch(() =>
        setTokenStatus({
          configured: false,
          hint: "无法读取 Token 状态",
        }),
      );
  }, []);

  async function saveToken() {
    setSaving(true);
    setTokenMsg(null);
    try {
      const st = await setTushareToken(tokenInput.trim());
      setTokenStatus(st);
      setTokenInput("");
      setTokenMsg(st.configured ? "已保存" : "已清除");
    } catch (e) {
      setTokenMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function exportBackup() {
    setBackupBusy(true);
    setBackupMsg(null);
    try {
      const json = await exportUserData();
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
      a.href = url;
      a.download = `stock-predict-backup-${stamp}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setBackupMsg("已导出用户数据备份");
    } catch (e) {
      setBackupMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBackupBusy(false);
    }
  }

  async function importBackup(file: File) {
    setBackupBusy(true);
    setBackupMsg(null);
    try {
      const text = await file.text();
      const snap = await importUserData(text);
      applyImportedSnapshotFlag();
      applyUserSnapshot(snap);
      setBackupMsg("导入成功，自选与配置已恢复");
    } catch (e) {
      setBackupMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBackupBusy(false);
    }
  }

  return (
    <div className="h-full min-h-0 overflow-y-auto p-6 lg:p-8">
      <motion.header
        initial={{ opacity: 0, y: -12 }}
        animate={{ opacity: 1, y: 0 }}
        className="mb-8"
      >
        <h1 className="text-2xl font-semibold text-slate-100">设置</h1>
        <p className="mt-2 text-sm text-slate-400">
          回看天数可在预测页调整。自选与信号组合保存在应用数据目录，升级不会丢失；可在下方导出/导入备份。
        </p>
      </motion.header>

      <section className="mb-8">
        <h2 className="mb-3 text-sm font-medium text-slate-300">历史回看天数</h2>
        <p className="mb-3 text-xs text-slate-500">
          用最近多少个交易日的数据计算技术因子并做滚动回测。天数越长趋势越平滑，越短越敏感。
        </p>
        <div className="flex flex-wrap gap-2">
          {lookbackOptions.map((days) => (
            <button
              key={days}
              type="button"
              onClick={() => setLookbackDays(days)}
              className={cn(
                "rounded-xl border px-4 py-2.5 text-sm font-medium transition",
                lookbackDays === days
                  ? "border-cyan-500/40 bg-cyan-500/10 text-cyan-300"
                  : "border-white/5 bg-slate-900/50 text-slate-400 hover:border-white/10 hover:text-slate-200",
              )}
            >
              {days} 日
            </button>
          ))}
        </div>
      </section>

      <section className="mb-8">
        <h2 className="mb-3 text-sm font-medium text-slate-300">Tushare Token（资金流）</h2>
        <p className="mb-3 text-xs text-slate-500">
          「资金流(主力)」默认用腾讯两市成交额做免费可回测代理。配置 Tushare（
          <code className="text-slate-400">moneyflow_mkt_dc</code>
          ）后自动升级为真·大盘主力净流入。也可设环境变量{" "}
          <code className="text-slate-400">TUSHARE_TOKEN</code>。
        </p>
        <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-4">
          <p
            className={cn(
              "mb-3 text-xs",
              tokenStatus?.configured ? "text-emerald-400" : "text-amber-400/90",
            )}
          >
            {tokenStatus?.hint ?? "读取中…"}
          </p>
          <div className="flex flex-col gap-2 sm:flex-row">
            <input
              type="password"
              autoComplete="off"
              placeholder={tokenStatus?.configured ? "已配置，输入新 Token 可覆盖" : "粘贴 Tushare Token"}
              value={tokenInput}
              onChange={(e) => setTokenInput(e.target.value)}
              className="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-950/60 px-3 py-2.5 text-sm text-slate-200 outline-none placeholder:text-slate-600 focus:border-cyan-500/40"
            />
            <button
              type="button"
              disabled={saving}
              onClick={() => void saveToken()}
              className="rounded-xl border border-cyan-500/30 bg-cyan-500/10 px-4 py-2.5 text-sm font-medium text-cyan-300 transition hover:bg-cyan-500/20 disabled:opacity-50"
            >
              {saving ? "保存中…" : "保存"}
            </button>
            <button
              type="button"
              disabled={saving}
              onClick={() => {
                setTokenInput("");
                void setTushareToken("").then(setTokenStatus);
                setTokenMsg("已清除");
              }}
              className="rounded-xl border border-white/10 px-4 py-2.5 text-sm text-slate-400 transition hover:border-white/20 hover:text-slate-200 disabled:opacity-50"
            >
              清除
            </button>
          </div>
          {tokenMsg && <p className="mt-2 text-xs text-slate-500">{tokenMsg}</p>}
        </div>
      </section>

      <section className="mb-8">
        <h2 className="mb-3 text-sm font-medium text-slate-300">数据备份</h2>
        <p className="mb-3 text-xs text-slate-500">
          自选、策略组合、盯盘规则与偏好保存在应用数据目录（升级安装包不会清除）。可导出 JSON
          换机恢复；导入前会自动备份当前库。
        </p>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={backupBusy}
            onClick={() => void exportBackup()}
            className="rounded-xl border border-cyan-500/30 bg-cyan-500/10 px-4 py-2.5 text-sm font-medium text-cyan-300 transition hover:bg-cyan-500/20 disabled:opacity-50"
          >
            {backupBusy ? "处理中…" : "导出备份"}
          </button>
          <label className="cursor-pointer rounded-xl border border-white/10 px-4 py-2.5 text-sm text-slate-300 transition hover:border-white/20 hover:text-slate-100">
            导入备份
            <input
              type="file"
              accept="application/json,.json"
              className="hidden"
              disabled={backupBusy}
              onChange={(e) => {
                const f = e.target.files?.[0];
                e.target.value = "";
                if (f) void importBackup(f);
              }}
            />
          </label>
        </div>
        {backupMsg && <p className="mt-2 text-xs text-slate-500">{backupMsg}</p>}
      </section>

      <section className="mb-8">
        <h2 className="mb-3 text-sm font-medium text-slate-300">盯盘助手</h2>
        <div className="rounded-2xl border border-white/5 bg-slate-900/50 p-4 text-xs leading-relaxed text-slate-500">
          <p>
            在「自选」页开启盯盘后，应用会在交易时段轮询行情；触达预警时弹出<strong className="font-medium text-slate-400">系统通知</strong>。
          </p>
          <p className="mt-2">
            Android 锁屏监控依赖前台服务，状态栏会常驻「盯盘中」通知。若厂商省电限制后台，请在系统设置中允许本应用后台运行/关闭电池优化。强制停止应用后需重新开启盯盘。
          </p>
        </div>
      </section>

      <section>
        <h2 className="mb-3 text-sm font-medium text-slate-300">预测算法</h2>
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
      </section>

      <div className="mt-8 rounded-2xl border border-white/5 bg-slate-900/30 p-5 text-sm text-slate-500">
        <p className="font-medium text-slate-400">扩展说明</p>
        <ul className="mt-3 list-inside list-disc space-y-1.5 leading-relaxed">
          <li>因子模型：<code className="text-slate-400">src-tauri/src/factor_model.rs</code></li>
          <li>资金流：<code className="text-slate-400">src-tauri/src/capital_flow.rs</code></li>
          <li>股票列表：<code className="text-slate-400">src-tauri/resources/stocks.json</code></li>
        </ul>
      </div>
    </div>
  );
}
