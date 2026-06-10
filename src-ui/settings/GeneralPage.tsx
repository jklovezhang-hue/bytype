import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { LANG_OPTIONS } from "./consts";
import type { PageProps } from "./types";
import { Row, Section, SelectBox, Toggle } from "./widgets";
import { getTheme, setTheme, THEME_OPTIONS, type Theme } from "./theme";

export default function GeneralPage({ cfg, set }: PageProps) {
  // 开机自启走 autostart 插件(注册表),立即生效,不进 config.toml、不参与脏检查。
  const [autoStart, setAutoStart] = useState(false);
  const [autoErr, setAutoErr] = useState<string | null>(null);
  const [autoBusy, setAutoBusy] = useState(false);
  const [theme, setThemeState] = useState<Theme>(getTheme());

  useEffect(() => {
    let alive = true;
    isEnabled()
      .then((v) => {
        if (alive) setAutoStart(v);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const toggleAutostart = async (v: boolean) => {
    if (autoBusy) return; // IPC 进行中忽略再次点击,避免乐观状态与注册表实际值竞态
    setAutoErr(null);
    setAutoBusy(true);
    setAutoStart(v); // 乐观切换
    try {
      if (v) await enable();
      else await disable();
    } catch (e) {
      setAutoStart(!v); // 失败回弹
      setAutoErr(String(e));
    } finally {
      setAutoBusy(false);
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
        <Toggle checked={autoStart} onChange={toggleAutostart} />
      </Row>
      {autoErr && <p className="text-xs text-red-600 dark:text-red-400 -mt-1">{autoErr}</p>}
      <Row label="外观" sub="界面配色,立即生效,无需保存">
        <SelectBox
          value={theme}
          onChange={(v) => {
            setTheme(v as Theme);
            setThemeState(v as Theme);
          }}
          options={THEME_OPTIONS}
        />
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
