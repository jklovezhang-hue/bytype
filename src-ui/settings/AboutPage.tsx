import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openConfigDir } from "./api";
import { Section } from "./widgets";
import iconUrl from "../../src-tauri/icons/icon.png";

const EMAIL = "jklover2025@outlook.com";

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
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板不可用时静默;用户可手动选中复制
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center gap-4">
        <img src={iconUrl} alt="ByType" className="w-14 h-14 rounded-xl" />
        <div>
          <div className="text-lg font-semibold text-neutral-900">
            ByType <span className="text-sm font-normal text-neutral-400 ml-1">v{version}</span>
          </div>
          <div className="text-sm text-neutral-500">按住热键说话,松手即出字 —— 本地识别 + LLM 整理。</div>
        </div>
      </div>
      <Section title="作者与联系">
        <p className="text-sm text-neutral-700">© 2026 Yong Zhang</p>
        <p className="text-sm text-neutral-700 flex items-center gap-2">
          <a className="text-blue-600 hover:underline" href={`mailto:${EMAIL}`}>
            {EMAIL}
          </a>
          <button
            type="button"
            onClick={copyEmail}
            className="text-xs px-2 py-0.5 rounded border border-neutral-300 text-neutral-500 hover:bg-neutral-50"
          >
            {copied ? "已复制 ✓" : "复制"}
          </button>
        </p>
      </Section>
      <Section title="配置文件">
        <p className="text-xs text-neutral-500 break-all">
          {cfgPath ?? "未找到,保存后将创建于程序目录"}
        </p>
        <div>
          <button
            type="button"
            onClick={() => openConfigDir()}
            className="px-3 py-1.5 rounded-md border border-neutral-300 text-sm text-neutral-600 bg-white hover:bg-neutral-50"
          >
            打开所在文件夹
          </button>
        </div>
      </Section>
      <p className="text-xs text-neutral-400">第三方开源组件致谢将在正式安装版中提供。</p>
    </div>
  );
}
