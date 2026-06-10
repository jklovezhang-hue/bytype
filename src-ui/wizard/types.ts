export interface WizardState {
  ready: boolean;
  config_exists: boolean;
  model_present: boolean;
  model_dir: string;
}

export interface DepCheck {
  key: string;
  label: string;
  status: "ok" | "bad" | "warn";
  detail: string;
  fix_url: string | null;
}

export interface DlProgress {
  file: string; // "tokens" | "model"
  received: number;
  total: number;
}
