import type { AppStyle, PageProps } from "./types";
import { Section, TextInput } from "./widgets";

export default function AppStylePage({ cfg, set }: PageProps) {
  const rows = cfg.app_style;

  const update = (i: number, patch: Partial<AppStyle>) =>
    set((c) => ({
      ...c,
      app_style: c.app_style.map((r, j) => (j === i ? { ...r, ...patch } : r)),
    }));

  const remove = (i: number) =>
    set((c) => ({ ...c, app_style: c.app_style.filter((_, j) => j !== i) }));

  const addRow = () =>
    set((c) => ({ ...c, app_style: [...c.app_style, { match: "", style: "" }] }));

  return (
    <Section title="应用风格">
      <p className="text-xs text-neutral-400">
        前台进程名包含「匹配串」即生效(不区分大小写),取第一条命中;匹配串为空的行保存时自动忽略。
      </p>
      {rows.map((r, i) => (
        <div key={i} className="flex items-center gap-2">
          <div className="w-36 flex-none">
            <TextInput
              value={r.match}
              placeholder="如 outlook"
              onChange={(e) => update(i, { match: e.target.value })}
            />
          </div>
          <TextInput
            value={r.style}
            placeholder="如 用正式、专业的书面语。"
            onChange={(e) => update(i, { style: e.target.value })}
          />
          <button
            type="button"
            className="flex-none text-red-400 hover:text-red-600 dark:hover:text-red-400"
            title="删除"
            onClick={() => remove(i)}
          >
            🗑
          </button>
        </div>
      ))}
      <div>
        <button
          type="button"
          onClick={addRow}
          className="px-3 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm text-neutral-600 dark:text-neutral-300 bg-white dark:bg-neutral-800 hover:bg-neutral-50 dark:hover:bg-neutral-700"
        >
          + 添加规则
        </button>
      </div>
    </Section>
  );
}
