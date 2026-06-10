import { useEffect, useMemo, useState } from "react";
import { getConfig, restartApp, saveConfig } from "./settings/api";
import type { Config } from "./settings/types";
import { hotkeyConflict } from "./settings/validate";
import AboutPage from "./settings/AboutPage";
import AppStylePage from "./settings/AppStylePage";
import GeneralPage from "./settings/GeneralPage";
import HelpPage from "./settings/HelpPage";
import HotkeyPage from "./settings/HotkeyPage";
import LlmPage from "./settings/LlmPage";
import VocabPage from "./settings/VocabPage";

const PAGES = [
  { id: "general", icon: "⚙", label: "通用" },
  { id: "hotkey", icon: "⌨", label: "热键" },
  { id: "llm", icon: "✨", label: "LLM 整理" },
  { id: "vocab", icon: "📖", label: "词库" },
  { id: "style", icon: "🎯", label: "应用风格" },
  { id: "help", icon: "❓", label: "帮助" },
  { id: "about", icon: "ℹ️", label: "关于" },
] as const;
type PageId = (typeof PAGES)[number]["id"];

export default function App() {
  const [page, setPage] = useState<PageId>("general");
  const [cfg, setCfg] = useState<Config | null>(null);
  const [snapshot, setSnapshot] = useState(""); // 加载时的 JSON 快照,用于脏检查与"放弃更改"
  const [cfgPath, setCfgPath] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getConfig().then((r) => {
      setCfg(r.config);
      setSnapshot(JSON.stringify(r.config));
      setCfgPath(r.path);
      setLoadError(r.error);
    });
  }, []);

  const dirty = useMemo(
    () => cfg !== null && JSON.stringify(cfg) !== snapshot,
    [cfg, snapshot]
  );
  const conflict = cfg !== null && hotkeyConflict(cfg.hotkey);

  if (cfg === null) {
    return (
      <div className="h-screen flex items-center justify-center text-neutral-400 text-sm bg-white dark:bg-neutral-900">
        加载配置…
      </div>
    );
  }

  const set = (updater: (c: Config) => Config) => {
    setSaveError(null);
    setCfg((c) => (c ? updater(c) : c));
  };

  const onDiscard = () => {
    setCfg(JSON.parse(snapshot) as Config);
    setSaveError(null);
  };

  const onSave = async () => {
    setSaving(true);
    setSaveError(null);
    // 匹配串为空的应用风格行保存时忽略(spec)
    const toSave: Config = {
      ...cfg,
      app_style: cfg.app_style.filter((r) => r.match.trim() !== ""),
    };
    try {
      await saveConfig(toSave);
    } catch (e) {
      setSaveError(`保存失败:${e}`);
      setSaving(false);
      return;
    }
    try {
      await restartApp(); // 应用即将重启,这个 Promise 不会正常返回
    } catch (e) {
      setSaveError(`重启失败(配置已保存,请手动重启):${e}`);
      setSaving(false);
    }
  };

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-neutral-900 text-neutral-800 dark:text-neutral-200">
      <div className="flex-1 flex min-h-0">
        <nav className="w-44 flex-none bg-neutral-50 dark:bg-neutral-950 border-r border-neutral-200 dark:border-neutral-700 p-2.5 flex flex-col gap-1">
          {PAGES.map((p) => (
            <button
              key={p.id}
              onClick={() => setPage(p.id)}
              className={`text-left px-3 py-2 rounded-md text-sm ${
                page === p.id ? "bg-blue-500 text-white" : "text-neutral-600 dark:text-neutral-300 hover:bg-neutral-100 dark:hover:bg-neutral-800"
              }`}
            >
              {p.icon} {p.label}
            </button>
          ))}
        </nav>
        <main className="flex-1 min-w-0 overflow-y-auto px-6 py-5">
          {loadError && (
            <div className="mb-4 text-xs rounded-md border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-900/20 text-amber-800 dark:text-amber-300 px-3 py-2">
              config.toml 解析失败:{loadError} —— 以下显示默认值,保存将整文件覆盖。
            </div>
          )}
          {!loadError && cfgPath === null && (
            <div className="mb-4 text-xs rounded-md border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-900/20 text-amber-800 dark:text-amber-300 px-3 py-2">
              未找到 config.toml,保存时将在程序目录创建。
            </div>
          )}
          {page === "general" && <GeneralPage cfg={cfg} set={set} />}
          {page === "hotkey" && <HotkeyPage cfg={cfg} set={set} />}
          {page === "llm" && <LlmPage cfg={cfg} set={set} />}
          {page === "vocab" && <VocabPage cfg={cfg} set={set} />}
          {page === "style" && <AppStylePage cfg={cfg} set={set} />}
          {page === "help" && <HelpPage cfg={cfg} />}
          {page === "about" && <AboutPage cfgPath={cfgPath} />}
        </main>
      </div>
      {dirty && (
        <div className="flex-none border-t border-neutral-200 dark:border-neutral-700 bg-amber-50 dark:bg-neutral-800 px-4 py-2.5 flex items-center gap-3">
          <span className="text-sm text-amber-700 dark:text-amber-400">● 有未保存的更改</span>
          {conflict && <span className="text-xs text-red-600 dark:text-red-400">热键互相冲突,无法保存</span>}
          {saveError && (
            <span className="text-xs text-red-600 dark:text-red-400 truncate">{saveError}</span>
          )}
          <span className="flex-1" />
          <span className="text-xs text-neutral-400">保存后 ByType 将自动重启</span>
          <button
            onClick={onDiscard}
            disabled={saving}
            className="px-3.5 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm text-neutral-600 dark:text-neutral-300 bg-white dark:bg-neutral-800 hover:bg-neutral-50 dark:hover:bg-neutral-700"
          >
            放弃更改
          </button>
          <button
            onClick={onSave}
            disabled={conflict || saving}
            className="px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {saving ? "保存中…" : "保存并重启"}
          </button>
        </div>
      )}
    </div>
  );
}
