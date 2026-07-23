import { useState } from "react";
import type { Stock } from "@/types";
import { useStockStore } from "@/stores/stockStore";
import { GROUP_HOLDINGS_MIRROR, newGroupId } from "@/lib/pool";
import { cn } from "@/lib/utils";

export function AddToPoolModal({
  stock,
  onClose,
  defaultGroupId,
}: {
  stock: Stock;
  onClose: () => void;
  defaultGroupId?: string;
}) {
  const poolGroups = useStockStore((s) => s.poolGroups);
  const addToPool = useStockStore((s) => s.addToPool);
  const upsertPoolGroup = useStockStore((s) => s.upsertPoolGroup);
  const userGroups = poolGroups.filter((g) => g.kind !== "mirror");
  const [groupId, setGroupId] = useState(
    defaultGroupId ?? userGroups[0]?.id ?? "g_watch",
  );
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 p-4 sm:items-center"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="w-full max-w-md rounded-2xl border border-white/10 bg-slate-900 p-4 shadow-xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal
      >
        <h2 className="text-lg font-medium text-slate-100">入池</h2>
        <p className="mt-1 text-xs text-slate-500">
          {stock.name} · {stock.code}
        </p>

        {!creating ? (
          <div className="mt-3 space-y-2">
            {userGroups
              .filter((g) => g.id !== GROUP_HOLDINGS_MIRROR)
              .map((g) => (
                <button
                  key={g.id}
                  type="button"
                  onClick={() => setGroupId(g.id)}
                  className={cn(
                    "flex w-full items-center justify-between rounded-xl border px-3 py-2 text-sm",
                    groupId === g.id
                      ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-200"
                      : "border-white/5 text-slate-300",
                  )}
                >
                  {g.name}
                  {groupId === g.id && <span>✓</span>}
                </button>
              ))}
            <button
              type="button"
              onClick={() => setCreating(true)}
              className="text-xs text-cyan-400"
            >
              + 新建分组
            </button>
          </div>
        ) : (
          <div className="mt-3 space-y-2">
            <input
              className="w-full rounded-xl border border-white/10 bg-slate-950 px-3 py-2 text-sm text-slate-200"
              placeholder="分组名称"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
            />
            <button
              type="button"
              className="text-xs text-slate-400"
              onClick={() => setCreating(false)}
            >
              返回选择
            </button>
          </div>
        )}

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            className="rounded-xl px-3 py-2 text-sm text-slate-400"
            onClick={onClose}
          >
            取消
          </button>
          <button
            type="button"
            className="rounded-xl bg-emerald-500/20 px-3 py-2 text-sm text-emerald-300 ring-1 ring-emerald-500/40"
            onClick={() => {
              let gid = groupId;
              if (creating) {
                const name = newName.trim();
                if (!name) return;
                gid = newGroupId();
                upsertPoolGroup({
                  id: gid,
                  name,
                  sortOrder: poolGroups.length,
                  kind: "user",
                });
              }
              addToPool(stock, gid);
              onClose();
            }}
          >
            确认入池
          </button>
        </div>
      </div>
    </div>
  );
}
