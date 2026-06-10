import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DepCheck, DlProgress, WizardState } from "./types";
import type { LlmConfig } from "../settings/types";

export const wizardState = () => invoke<WizardState>("wizard_state");
export const checkDependencies = () => invoke<DepCheck[]>("check_dependencies");
export const downloadModel = (modelUrl: string, tokensUrl: string) =>
  invoke<void>("download_model", { modelUrl, tokensUrl });
export const cancelDownload = () => invoke<void>("cancel_download");
// Tauri 2 命令参数:JS 端用 camelCase,自动映射到 Rust 的 snake_case(model_path/tokens_path)。
export const importModel = (modelPath: string, tokensPath: string) =>
  invoke<void>("import_model", { modelPath, tokensPath });
export const finishWizard = (llm: LlmConfig) => invoke<void>("finish_wizard", { llm });
export const openExternal = (url: string) => invoke<void>("open_external", { url });

export const onDlProgress = (cb: (p: DlProgress) => void): Promise<UnlistenFn> =>
  listen<DlProgress>("bt:dl-progress", (e) => cb(e.payload));
