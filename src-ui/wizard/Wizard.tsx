import { useEffect, useState } from "react";
import { finishWizard } from "./api";
import { getConfig } from "../settings/api";
import type { LlmConfig } from "../settings/types";
import type { WizardState } from "./types";
import WelcomeStep from "./WelcomeStep";
import DepsStep from "./DepsStep";
import LlmStep from "./LlmStep";
import DownloadStep from "./DownloadStep";
import DoneStep from "./DoneStep";

const STEPS = ["欢迎", "依赖检测", "LLM 配置", "下载模型", "完成"];

export default function Wizard({ initial }: { initial: WizardState }) {
  const [step, setStep] = useState(0);
  const [llm, setLlm] = useState<LlmConfig | null>(null);
  const [depsOk, setDepsOk] = useState(false);
  const [modelReady, setModelReady] = useState(initial.model_present);
  const [finishing, setFinishing] = useState(false);
  const [finishErr, setFinishErr] = useState<string | null>(null);

  useEffect(() => {
    // 预填现有 LLM(config 不存在时后端返回默认),避免覆盖已有配置。
    getConfig()
      .then((r) => setLlm(r.config.llm))
      .catch(() => {});
  }, []);

  const canNext = step === 1 ? depsOk : step === 3 ? modelReady : true;
  const next = () => setStep((s) => Math.min(STEPS.length - 1, s + 1));
  const prev = () => setStep((s) => Math.max(0, s - 1));

  const onFinish = async () => {
    if (!llm) return;
    setFinishing(true);
    setFinishErr(null);
    try {
      await finishWizard(llm); // 成功后后端隐藏主窗口
    } catch (e) {
      setFinishErr(String(e));
      setFinishing(false);
    }
  };

  return (
    <div className="h-screen flex flex-col bg-white text-neutral-800 dark:bg-neutral-900 dark:text-neutral-200">
      <div className="flex gap-1 px-4 py-3 text-xs border-b border-neutral-200 dark:border-neutral-700 bg-neutral-50 dark:bg-neutral-950">
        {STEPS.map((s, i) => (
          <span
            key={s}
            className={`px-2 ${
              i === step
                ? "text-blue-500 font-semibold"
                : i < step
                ? "text-emerald-600"
                : "text-neutral-400"
            }`}
          >
            {i < step ? "✓ " : `${i + 1} `}
            {s}
          </span>
        ))}
      </div>
      <div className="flex-1 overflow-y-auto px-6 py-5">
        {step === 0 && <WelcomeStep />}
        {step === 1 && <DepsStep onStatus={setDepsOk} />}
        {step === 2 && llm && <LlmStep llm={llm} setLlm={setLlm} />}
        {step === 3 && <DownloadStep modelReady={modelReady} onReady={() => setModelReady(true)} />}
        {step === 4 && <DoneStep />}
      </div>
      <div className="flex-none border-t border-neutral-200 dark:border-neutral-700 px-4 py-2.5 flex items-center gap-3">
        {step > 0 && step < 4 && (
          <button
            onClick={prev}
            className="px-3.5 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm"
          >
            ← 上一步
          </button>
        )}
        {finishErr && <span className="text-xs text-red-600">{finishErr}</span>}
        <span className="flex-1" />
        {step < 4 && (
          <button
            onClick={next}
            disabled={!canNext}
            className="px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-40"
          >
            下一步 →
          </button>
        )}
        {step === 4 && (
          <button
            onClick={onFinish}
            disabled={finishing || !llm}
            className="px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-40"
          >
            {finishing ? "启动中…" : "完成,开始使用"}
          </button>
        )}
      </div>
    </div>
  );
}
