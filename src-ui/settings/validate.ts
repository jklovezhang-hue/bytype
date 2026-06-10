import type { HotkeyConfig } from "./types";

/** 三个热键任意两个相同(大小写不敏感)即冲突。 */
export function hotkeyConflict(h: HotkeyConfig): boolean {
  const keys = [h.primary, h.translate_modifier, h.command_modifier].map((s) =>
    s.trim().toLowerCase()
  );
  return new Set(keys).size < 3;
}
