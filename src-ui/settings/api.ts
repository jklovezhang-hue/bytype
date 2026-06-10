import { invoke } from "@tauri-apps/api/core";
import type { Config, GetConfigResp, LlmConfig, TestOk } from "./types";

export const getConfig = () => invoke<GetConfigResp>("get_config");
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });
export const testLlm = (llm: LlmConfig) => invoke<TestOk>("test_llm", { llm });
export const restartApp = () => invoke<void>("restart_app");
export const openConfigDir = () => invoke<void>("open_config_dir");
export const openExternal = (url: string) => invoke<void>("open_external", { url });
