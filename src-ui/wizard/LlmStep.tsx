import { useState } from "react";
import { testLlm } from "../settings/api";
import type { LlmConfig } from "../settings/types";

export default function LlmStep({
  llm,
  setLlm,
}: {
  llm: LlmConfig;
  setLlm: (l: LlmConfig) => void;
}) {
  const [show, setShow] = useState(false);
  const [test, setTest] = useState<string | null>(null);
  const set = (p: Partial<LlmConfig>) => setLlm({ ...llm, ...p });
  const cls =
    "border border-neutral-300 dark:border-neutral-700 dark:bg-neutral-800 rounded-md px-2.5 py-1.5 text-sm";

  const runTest = async () => {
    setTest("测试中…");
    try {
      const r = await testLlm({ ...llm, enabled: true });
      setTest(`✓ ${r.latency_ms}ms · ${r.reply.slice(0, 40)}`);
    } catch (e) {
      setTest(`✗ ${String(e).slice(0, 80)}`);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold">LLM 中转站(用于整理/翻译,可跳过)</h2>
      <label className="text-sm flex flex-col gap-1">
        接口地址
        <input
          className={cls}
          value={llm.base_url}
          placeholder="https://example.com/v1"
          onChange={(e) => set({ base_url: e.target.value })}
        />
      </label>
      <label className="text-sm flex flex-col gap-1">
        API Key
        <span className="flex items-center gap-1.5">
          <input
            type={show ? "text" : "password"}
            className={`flex-1 ${cls}`}
            value={llm.api_key}
            onChange={(e) => set({ api_key: e.target.value })}
          />
          <button type="button" onClick={() => setShow(!show)} className="text-neutral-400">
            👁
          </button>
        </span>
      </label>
      <label className="text-sm flex flex-col gap-1">
        模型
        <input
          className={cls}
          value={llm.model}
          placeholder="deepseek-v4-flash"
          onChange={(e) => set({ model: e.target.value })}
        />
      </label>
      <div className="flex items-center gap-3">
        <button
          onClick={runTest}
          className="px-3 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm"
        >
          ⚡ 测试连接
        </button>
        {test && <span className="text-xs text-neutral-500">{test}</span>}
      </div>
      <p className="text-xs text-neutral-400">不填也能用:听写直接输出原始识别文本,日后可在设置里补。</p>
    </div>
  );
}
