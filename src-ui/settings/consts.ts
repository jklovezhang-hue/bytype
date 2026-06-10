// 与 src/keys.rs 支持的 8 个键名保持一致。
export const KEY_OPTIONS = [
  { value: "LWin", label: "左 Win" },
  { value: "RWin", label: "右 Win" },
  { value: "LAlt", label: "左 Alt" },
  { value: "RAlt", label: "右 Alt" },
  { value: "LCtrl", label: "左 Ctrl" },
  { value: "RCtrl", label: "右 Ctrl" },
  { value: "LShift", label: "左 Shift" },
  { value: "RShift", label: "右 Shift" },
];

/** 键名 → 中文标签;未知键名原样返回(配置可能手写了别的值)。 */
export const keyLabel = (v: string) =>
  KEY_OPTIONS.find((k) => k.value.toLowerCase() === v.trim().toLowerCase())?.label ?? v;

export const LANG_OPTIONS = [
  { value: "auto", label: "自动" },
  { value: "zh", label: "中文" },
  { value: "en", label: "英文" },
  { value: "yue", label: "粤语" },
  { value: "ja", label: "日语" },
  { value: "ko", label: "韩语" },
];

export const MODE_OPTIONS = [
  { value: "clean", label: "忠实清理" },
  { value: "polish", label: "智能整理" },
  { value: "summary", label: "要点提炼" },
];
