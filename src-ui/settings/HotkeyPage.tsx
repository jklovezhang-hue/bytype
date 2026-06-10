import { KEY_OPTIONS } from "./consts";
import type { PageProps } from "./types";
import { hotkeyConflict } from "./validate";
import { Row, Section, SelectBox } from "./widgets";

const ITEMS = [
  { key: "primary", label: "主键", sub: "按住说话,松手出字" },
  { key: "translate_modifier", label: "翻译键", sub: "主键+它:译成英文" },
  { key: "command_modifier", label: "命令键", sub: "主键+它:对选中文字执行语音命令" },
] as const;

export default function HotkeyPage({ cfg, set }: PageProps) {
  const conflict = hotkeyConflict(cfg.hotkey);
  return (
    <Section title="热键">
      {ITEMS.map((it) => (
        <Row key={it.key} label={it.label} sub={it.sub}>
          <div className={conflict ? "rounded-md ring-2 ring-red-400" : ""}>
            <SelectBox
              value={cfg.hotkey[it.key]}
              onChange={(v) => set((c) => ({ ...c, hotkey: { ...c.hotkey, [it.key]: v } }))}
              options={KEY_OPTIONS}
            />
          </div>
        </Row>
      ))}
      {conflict && <p className="text-xs text-red-600">三个热键必须互不相同。</p>}
      <p className="text-xs text-neutral-400">
        修饰键要在主键按住期间一起按下;录音中按 Esc 或点浮窗药丸可取消。
      </p>
    </Section>
  );
}
