import { useState } from "react";
import type { PageProps } from "./types";
import { Section, TextInput } from "./widgets";

export default function VocabPage({ cfg, set }: PageProps) {
  const [draft, setDraft] = useState("");
  const words = cfg.llm.vocabulary;

  const add = () => {
    const w = draft.trim();
    if (!w) return;
    if (!words.includes(w)) {
      set((c) => ({ ...c, llm: { ...c.llm, vocabulary: [...c.llm.vocabulary, w] } }));
    }
    setDraft(""); // 重复词也清空输入,视觉上"已存在"
  };

  const remove = (i: number) =>
    set((c) => ({ ...c, llm: { ...c.llm, vocabulary: c.llm.vocabulary.filter((_, j) => j !== i) } }));

  return (
    <Section title="词库">
      <p className="text-xs text-neutral-400">
        专有名词优先按以下拼写输出(如 Kubernetes、OneDrive)。输入后按回车添加,点 × 删除。
      </p>
      <div className="flex flex-wrap items-center gap-2">
        {words.map((w, i) => (
          <span
            key={`${w}-${i}`}
            className="inline-flex items-center gap-1.5 rounded-full border border-blue-200 bg-blue-50 text-blue-700 text-sm px-3 py-1"
          >
            {w}
            <button type="button" className="text-blue-400 hover:text-blue-700" onClick={() => remove(i)}>
              ×
            </button>
          </span>
        ))}
        <div className="w-44">
          <TextInput
            value={draft}
            placeholder="输入后按回车…"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
          />
        </div>
      </div>
    </Section>
  );
}
