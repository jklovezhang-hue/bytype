// 外观主题:跟随系统/浅色/深色。立即生效,localStorage 持久化,
// 不进 config.toml、不参与脏检查/保存栏(与开机自启同类)。
export type Theme = "system" | "light" | "dark";

export const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
];

const KEY = "bt-theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: Theme = read();

function read(): Theme {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" ? v : "system";
}

function apply(t: Theme) {
  const dark = t === "dark" || (t === "system" && media.matches);
  document.documentElement.classList.toggle("dark", dark);
}

export function getTheme(): Theme {
  return current;
}

export function setTheme(t: Theme) {
  current = t;
  localStorage.setItem(KEY, t);
  apply(t);
}

/** 应用启动时调用一次:应用当前主题,并在「跟随系统」时联动系统切换。 */
export function initTheme() {
  apply(current);
  media.addEventListener("change", () => apply(current));
}
