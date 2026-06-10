import React, { useState } from "react";

/** 页面区块:标题 + 纵向内容。 */
export function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold text-neutral-900 dark:text-neutral-100">{title}</h2>
      {children}
    </div>
  );
}

/** 设置行:左标签(+小字说明),右控件。 */
export function Row({ label, sub, children }: { label: string; sub?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-1">
      <div className="min-w-0">
        <div className="text-sm text-neutral-800 dark:text-neutral-200">{label}</div>
        {sub && <div className="text-xs text-neutral-400 mt-0.5">{sub}</div>}
      </div>
      <div className="flex-none flex items-center gap-2">{children}</div>
    </div>
  );
}

export function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`w-10 h-[22px] rounded-full relative transition-colors ${checked ? "bg-blue-500" : "bg-neutral-300 dark:bg-neutral-600"}`}
    >
      <span
        className={`absolute top-[2px] w-[18px] h-[18px] rounded-full bg-white transition-all ${checked ? "right-[2px]" : "left-[2px]"}`}
      />
    </button>
  );
}

/** 文本输入:透传原生属性(value/onChange/type/placeholder/onKeyDown…)。 */
export function TextInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...props}
      className={`border border-neutral-300 dark:border-neutral-700 rounded-md px-2.5 py-1.5 text-sm w-full focus:outline-none focus:border-blue-500 dark:bg-neutral-800 dark:text-neutral-200 ${props.className ?? ""}`}
    />
  );
}

export function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  // 本地保留原始输入串:用户删光时不立刻上抛 0,失焦后回弹为外部值。
  const [raw, setRaw] = useState(String(value));
  React.useEffect(() => {
    setRaw(String(value));
  }, [value]);
  return (
    <input
      type="number"
      value={raw}
      min={min}
      max={max}
      step={step}
      onChange={(e) => {
        setRaw(e.target.value);
        const n = Number(e.target.value);
        if (e.target.value !== "" && !Number.isNaN(n)) onChange(n);
      }}
      onBlur={() => setRaw(String(value))}
      className="border border-neutral-300 dark:border-neutral-700 rounded-md px-2.5 py-1.5 text-sm w-24 focus:outline-none focus:border-blue-500 dark:bg-neutral-800 dark:text-neutral-200"
    />
  );
}

/** 下拉框;当前值不在选项里时(配置手写了未知值)原样保留为首项,避免静默改值。 */
export function SelectBox({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  const matched = options.find((o) => o.value.toLowerCase() === value.trim().toLowerCase());
  return (
    <select
      value={matched ? matched.value : value}
      onChange={(e) => onChange(e.target.value)}
      className="border border-neutral-300 dark:border-neutral-700 rounded-md px-2 py-1.5 text-sm bg-white dark:bg-neutral-800 dark:text-neutral-200 focus:outline-none focus:border-blue-500"
    >
      {!matched && <option value={value}>{value}</option>}
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Collapsible({ title, children }: { title: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="border-t border-dashed border-neutral-200 dark:border-neutral-700 pt-3">
      <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="text-sm text-neutral-500 dark:text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-300">
        {open ? "▾" : "▸"} {title}
      </button>
      {open && <div className="mt-3 flex flex-col gap-3">{children}</div>}
    </div>
  );
}
