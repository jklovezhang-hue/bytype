import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { LANG_OPTIONS } from "./consts";
import type { PageProps } from "./types";
import { Row, Section, SelectBox, Toggle } from "./widgets";

export default function GeneralPage({ cfg, set }: PageProps) {
  // 开机自启走 autostart 插件(注册表),立即生效,不进 config.toml、不参与脏检查。
  const [autoStart, setAutoStart] = useState(false);
  const [autoErr, setAutoErr] = useState<string | null>(null);

  useEffect(() => {
    isEnabled().then(setAutoStart).catch(() => {});
  }, []);

  const toggleAutostart = async (v: boolean) => {
    setAutoErr(null);
    setAutoStart(v); // 乐观切换
    try {
      if (v) await enable();
      else await disable();
    } catch (e) {
      setAutoStart(!v); // 失败回弹
      setAutoErr(String(e));
    }
  };

  return (
    <Section title="通用">
      <Row label="录音浮窗" sub="录音时屏幕底部显示计时药丸">
        <Toggle
          checked={cfg.overlay.enabled}
          onChange={(v) => set((c) => ({ ...c, overlay: { ...c.overlay, enabled: v } }))}
        />
      </Row>
      <Row label="提示音" sub="录音开始/结束播放提示音">
        <Toggle
          checked={cfg.sound.enabled}
          onChange={(v) => set((c) => ({ ...c, sound: { ...c.sound, enabled: v } }))}
        />
      </Row>
      <Row label="开机自启" sub="登录 Windows 后自动在后台运行(立即生效,无需保存)">
        {autoErr && <span className="text-xs text-red-600">{autoErr}</span>}
        <Toggle checked={autoStart} onChange={toggleAutostart} />
      </Row>
      <Row label="识别语言" sub="SenseVoice 识别语种">
        <SelectBox
          value={cfg.asr.language}
          onChange={(v) => set((c) => ({ ...c, asr: { ...c.asr, language: v } }))}
          options={LANG_OPTIONS}
        />
      </Row>
    </Section>
  );
}
