// 与 src/config.rs 的 serde JSON 字段一一对应(snake_case,零映射)。
// 注意 AppStyle 的 Rust 字段 match_ 经 #[serde(rename = "match")] 在 JSON 里是 match。

export interface HotkeyConfig {
  primary: string;
  translate_modifier: string;
  command_modifier: string;
}

export interface AsrConfig {
  model_dir: string;
  language: string;
}

export interface LlmConfig {
  enabled: boolean;
  base_url: string;
  api_key: string;
  model: string;
  mode: string;
  system_prompt: string;
  translate_prompt: string;
  command_prompt: string;
  temperature: number; // Rust 端为 f32;UI 步进 0.1,不会出现精度往返问题
  timeout_secs: number;
  skip_if_shorter_than: number;
  vocabulary: string[];
}

export interface InjectConfig {
  mode: string;
}

export interface AppStyle {
  match: string;
  style: string;
}

export interface OverlayConfig {
  enabled: boolean;
}

export interface SoundConfig {
  enabled: boolean;
  start_sound: string;
  end_sound: string;
}

export interface ModelConfig {
  model_url: string;
  tokens_url: string;
}

export interface Config {
  hotkey: HotkeyConfig;
  asr: AsrConfig;
  llm: LlmConfig;
  inject: InjectConfig;
  app_style: AppStyle[];
  overlay: OverlayConfig;
  sound: SoundConfig;
  model: ModelConfig;
}

export interface GetConfigResp {
  config: Config;
  path: string | null;
  error: string | null;
}

export interface TestOk {
  latency_ms: number;
  reply: string;
}

/** 各设置页的统一 props:配置 + 不可变更新器。 */
export interface PageProps {
  cfg: Config;
  set: (updater: (c: Config) => Config) => void;
}
