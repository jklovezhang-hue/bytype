import { useEffect, useState } from "react";
import { checkDependencies, openExternal } from "./api";
import type { DepCheck } from "./types";

export default function DepsStep({ onStatus }: { onStatus: (ok: boolean) => void }) {
  const [deps, setDeps] = useState<DepCheck[] | null>(null);

  const run = () => {
    setDeps(null);
    checkDependencies()
      .then((d) => {
        setDeps(d);
        onStatus(d.every((x) => x.status !== "bad")); // 无致命项即可继续
      })
      .catch(() => onStatus(false));
  };
  useEffect(run, []);

  const icon = (s: string) => (s === "ok" ? "✓" : s === "bad" ? "✕" : "!");
  const color = (s: string) =>
    s === "ok" ? "text-emerald-600" : s === "bad" ? "text-red-600" : "text-amber-600";

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold">运行环境检测</h2>
      {deps === null && <p className="text-sm text-neutral-400">检测中…</p>}
      {deps?.map((d) => (
        <div
          key={d.key}
          className="flex items-start gap-3 border border-neutral-200 dark:border-neutral-700 rounded-lg px-3 py-2"
        >
          <span className={`${color(d.status)} font-bold`}>{icon(d.status)}</span>
          <div className="flex-1 min-w-0">
            <div className="text-sm">{d.label}</div>
            <div className="text-xs text-neutral-400">{d.detail}</div>
            {d.fix_url && (
              <button
                onClick={() => openExternal(d.fix_url!)}
                className="text-xs text-blue-600 hover:underline mt-0.5"
              >
                {d.fix_url.startsWith("ms-settings:") ? "打开 Windows 设置" : "下载安装"}
              </button>
            )}
          </div>
        </div>
      ))}
      {deps && (
        <button onClick={run} className="self-start text-xs text-blue-600 hover:underline">
          重新检测
        </button>
      )}
      {deps?.some((d) => d.status === "bad") && (
        <p className="text-xs text-red-600">存在致命缺失,修复后点「重新检测」才能继续。</p>
      )}
    </div>
  );
}
