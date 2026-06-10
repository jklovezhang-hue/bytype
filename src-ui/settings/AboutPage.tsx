import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openConfigDir, openExternal } from "./api";
import { Section } from "./widgets";
import iconUrl from "../../src-tauri/icons/icon.png";

const EMAIL = "jklover2025@outlook.com";

const CREDITS: { name: string; license: string; url: string }[] = [
  { name: "ONNX Runtime", license: "MIT", url: "https://github.com/microsoft/onnxruntime" },
  { name: "sherpa-onnx", license: "Apache-2.0", url: "https://github.com/k2-fsa/sherpa-onnx" },
  { name: "SenseVoice 语音识别模型", license: "见上游许可", url: "https://github.com/FunAudioLLM/SenseVoice" },
  { name: "Tauri", license: "Apache-2.0 / MIT", url: "https://github.com/tauri-apps/tauri" },
  { name: "React", license: "MIT", url: "https://react.dev" },
  { name: "Tailwind CSS", license: "MIT", url: "https://tailwindcss.com" },
  { name: "Vite", license: "MIT", url: "https://vitejs.dev" },
];

export default function AboutPage({ cfgPath }: { cfgPath: string | null }) {
  const [version, setVersion] = useState("…");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion("未知"));
  }, []);

  const copyEmail = async () => {
    try {
      await navigator.clipboard.writeText(EMAIL);
      setCopied(true);
    } catch {
      // 剪贴板不可用时静默;用户可手动选中复制
    }
  };

  // 「已复制 ✓」1.5 秒后还原;组件卸载时取消计时器。
  useEffect(() => {
    if (!copied) return;
    const id = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(id);
  }, [copied]);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center gap-4">
        <img src={iconUrl} alt="ByType" className="w-14 h-14 rounded-xl" />
        <div>
          <div className="text-lg font-semibold text-neutral-900 dark:text-neutral-100">
            ByType <span className="text-sm font-normal text-neutral-400 ml-1">v{version}</span>
          </div>
          <div className="text-sm text-neutral-500 dark:text-neutral-400">按住热键说话,松手即出字 —— 本地识别 + LLM 整理。</div>
        </div>
      </div>
      <Section title="作者与联系">
        <p className="text-sm text-neutral-700 dark:text-neutral-300">© 2026 Yong Zhang</p>
        <p className="text-sm text-neutral-700 dark:text-neutral-300">本软件以 MIT 许可证开源</p>
        <p className="text-sm text-neutral-700 dark:text-neutral-300 flex items-center gap-2">
          <a className="text-blue-600 dark:text-blue-400 hover:underline" href={`mailto:${EMAIL}`}>
            {EMAIL}
          </a>
          <button
            type="button"
            onClick={copyEmail}
            className="text-xs px-2 py-0.5 rounded border border-neutral-300 dark:border-neutral-700 text-neutral-500 dark:text-neutral-400 hover:bg-neutral-50 dark:hover:bg-neutral-800"
          >
            {copied ? "已复制 ✓" : "复制"}
          </button>
        </p>
      </Section>
      <Section title="配置文件">
        <p className="text-xs text-neutral-500 dark:text-neutral-400 break-all">
          {cfgPath ?? "未找到,保存后将创建于程序目录"}
        </p>
        <div>
          <button
            type="button"
            onClick={() => openConfigDir()}
            className="px-3 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm text-neutral-600 dark:text-neutral-300 bg-white dark:bg-neutral-800 hover:bg-neutral-50 dark:hover:bg-neutral-700"
          >
            打开所在文件夹
          </button>
        </div>
      </Section>
      <Section title="第三方开源致谢">
        <ul className="flex flex-col gap-1.5">
          {CREDITS.map((c) => (
            <li
              key={c.name}
              className="text-sm text-neutral-700 dark:text-neutral-300 flex items-center gap-2 flex-wrap"
            >
              <span>{c.name}</span>
              <span className="text-xs text-neutral-400">{c.license}</span>
              <button
                type="button"
                onClick={() => openExternal(c.url)}
                className="text-xs text-blue-600 dark:text-blue-400 hover:underline"
              >
                打开
              </button>
            </li>
          ))}
        </ul>
        <p className="text-xs text-neutral-400 mt-2">
          以及 cpal、arboard、reqwest、serde 等众多 Rust / Node 开源库(MIT / Apache-2.0)。
        </p>
      </Section>
    </div>
  );
}
