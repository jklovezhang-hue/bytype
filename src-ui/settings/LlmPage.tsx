import { useState } from "react";
import { testLlm } from "./api";
import { MODE_OPTIONS } from "./consts";
import type { LlmConfig, PageProps } from "./types";
import { Collapsible, NumberInput, Row, Section, TextInput, Toggle } from "./widgets";

type TestState =
  | { st: "idle" }
  | { st: "testing" }
  | { st: "ok"; ms: number; reply: string }
  | { st: "err"; msg: string };

const PROMPTS = [
  { key: "system_prompt", label: "整理提示词" },
  { key: "translate_prompt", label: "翻译提示词" },
  { key: "command_prompt", label: "命令提示词" },
] as const;

export default function LlmPage({ cfg, set }: PageProps) {
  const [showKey, setShowKey] = useState(false);
  const [test, setTest] = useState<TestState>({ st: "idle" });
  const llm = cfg.llm;
  const setLlm = (patch: Partial<LlmConfig>) =>
    set((c) => ({ ...c, llm: { ...c.llm, ...patch } }));

  const runTest = async () => {
    setTest({ st: "testing" });
    try {
      const r = await testLlm(llm); // 用表单当前值,无需先保存
      setTest({ st: "ok", ms: r.latency_ms, reply: r.reply.slice(0, 50) });
    } catch (e) {
      setTest({ st: "err", msg: String(e).slice(0, 120) });
    }
  };

  return (
    <Section title="LLM 整理">
      <Row label="启用" sub="关闭则直接输出识别原文,不调用 LLM">
        <Toggle checked={llm.enabled} onChange={(v) => setLlm({ enabled: v })} />
      </Row>
      <Row label="接口地址" sub="OpenAI 兼容,通常以 /v1 结尾">
        <div className="w-72">
          <TextInput
            value={llm.base_url}
            placeholder="https://example.com/v1"
            onChange={(e) => setLlm({ base_url: e.target.value })}
          />
        </div>
      </Row>
      <Row label="API Key">
        <div className="w-72 flex items-center gap-1.5">
          <TextInput
            type={showKey ? "text" : "password"}
            value={llm.api_key}
            onChange={(e) => setLlm({ api_key: e.target.value })}
          />
          <button
            type="button"
            className="text-neutral-400 hover:text-neutral-600"
            title={showKey ? "隐藏" : "显示"}
            onClick={() => setShowKey(!showKey)}
          >
            👁
          </button>
        </div>
      </Row>
      <Row label="模型">
        <div className="w-72">
          <TextInput
            value={llm.model}
            placeholder="deepseek-v4-flash"
            onChange={(e) => setLlm({ model: e.target.value })}
          />
        </div>
      </Row>
      <Row label="整理力度" sub="忠实清理=只去口语词;智能整理=理顺+取自我更正;要点提炼=压缩成要点">
        <div className="flex border border-neutral-300 rounded-md overflow-hidden">
          {MODE_OPTIONS.map((m) => (
            <button
              key={m.value}
              type="button"
              onClick={() => setLlm({ mode: m.value })}
              className={`px-3 py-1.5 text-sm ${
                llm.mode === m.value
                  ? "bg-blue-500 text-white"
                  : "bg-white text-neutral-600 hover:bg-neutral-50"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>
      </Row>

      <div className="flex items-center gap-3 flex-wrap">
        <button
          type="button"
          onClick={runTest}
          disabled={test.st === "testing"}
          className="px-3.5 py-1.5 rounded-md border border-neutral-300 text-sm text-neutral-700 bg-white hover:bg-neutral-50 disabled:opacity-50"
        >
          ⚡ {test.st === "testing" ? "测试中…" : "测试连接"}
        </button>
        {test.st === "ok" && (
          <span className="text-xs text-emerald-600">
            ✓ 连接正常 · {test.ms}ms · 回复:「{test.reply}」
          </span>
        )}
        {test.st === "err" && <span className="text-xs text-red-600">✗ {test.msg}</span>}
        <span className="text-xs text-neutral-400">用当前表单值测试,无需先保存</span>
      </div>

      <Collapsible title="高级">
        <Row label="temperature" sub="0–2,越低输出越稳定">
          <NumberInput
            value={llm.temperature}
            onChange={(v) => setLlm({ temperature: Math.min(2, Math.max(0, v)) })}
            min={0}
            max={2}
            step={0.1}
          />
        </Row>
        <Row label="超时秒数">
          <NumberInput
            value={llm.timeout_secs}
            onChange={(v) => setLlm({ timeout_secs: Math.max(1, Math.round(v)) })}
            min={1}
          />
        </Row>
        <Row label="短文本跳过阈值" sub="识别文本字符数小于该值时不调用 LLM">
          <NumberInput
            value={llm.skip_if_shorter_than}
            onChange={(v) => setLlm({ skip_if_shorter_than: Math.max(0, Math.round(v)) })}
            min={0}
          />
        </Row>
        {PROMPTS.map((p) => (
          <div key={p.key} className="flex flex-col gap-1">
            <div className="text-sm text-neutral-800">{p.label}</div>
            <textarea
              value={llm[p.key]}
              rows={3}
              placeholder="留空使用内置预设"
              onChange={(e) => setLlm({ [p.key]: e.target.value } as Partial<LlmConfig>)}
              className="border border-neutral-300 rounded-md px-2.5 py-1.5 text-sm focus:outline-none focus:border-blue-500"
            />
          </div>
        ))}
      </Collapsible>
    </Section>
  );
}
