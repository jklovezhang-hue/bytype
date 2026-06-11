import { useEffect, useState } from "react";
import {
  deleteMeeting, getMeeting, listMeetings, openMeetingFolder, regenerateMinutes,
  type MeetingDetail, type MeetingSummary,
} from "./meetingApi";

export default function MeetingPage() {
  const [list, setList] = useState<MeetingSummary[]>([]);
  const [sel, setSel] = useState<MeetingDetail | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = () => listMeetings().then(setList).catch((e) => setErr(String(e)));
  useEffect(() => { refresh(); }, []);

  const open = async (base: string) => {
    setErr(null);
    setSel(await getMeeting(base));
  };
  const onRegen = async () => {
    if (!sel) return;
    setBusy("正在重新生成纪要…"); setErr(null);
    try {
      const md = await regenerateMinutes(sel.base);
      setSel({ ...sel, md });
    } catch (e) { setErr(String(e)); } finally { setBusy(null); }
  };
  const onCopy = () => { if (sel) navigator.clipboard.writeText(sel.md).catch(() => {}); };
  const onDelete = async () => {
    if (!sel) return;
    setBusy("删除中…");
    try { await deleteMeeting(sel.base); setSel(null); await refresh(); }
    catch (e) { setErr(String(e)); } finally { setBusy(null); }
  };

  return (
    <div className="flex gap-4 h-full">
      <aside className="w-52 flex-none overflow-y-auto border-r border-neutral-200 dark:border-neutral-700 pr-2">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-sm font-medium">历史会议</h2>
          <button onClick={refresh} className="text-xs text-blue-500 hover:underline">刷新</button>
        </div>
        {list.length === 0 && <p className="text-xs text-neutral-400">还没有会议记录</p>}
        {list.map((m) => (
          <button
            key={m.base}
            onClick={() => open(m.base)}
            className={`block w-full text-left px-2 py-1.5 rounded text-xs mb-1 ${
              sel?.base === m.base ? "bg-blue-500 text-white" : "hover:bg-neutral-100 dark:hover:bg-neutral-800"
            }`}
          >
            {m.base}
          </button>
        ))}
      </aside>
      <section className="flex-1 min-w-0 flex flex-col">
        {!sel && <p className="text-sm text-neutral-400">选择左侧的会议查看纪要与转写。</p>}
        {sel && (
          <>
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <span className="text-sm font-medium">{sel.base}</span>
              <span className="flex-1" />
              <button onClick={onCopy} className="text-xs px-2 py-1 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-800">复制</button>
              <button onClick={onRegen} disabled={!sel.has_json || !!busy} className="text-xs px-2 py-1 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-800 disabled:opacity-40">重新生成纪要</button>
              <button onClick={() => openMeetingFolder(sel.base)} className="text-xs px-2 py-1 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-800">打开文件夹</button>
              <button onClick={onDelete} disabled={!!busy} className="text-xs px-2 py-1 rounded border border-red-300 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 disabled:opacity-40">删除</button>
            </div>
            {busy && <p className="text-xs text-blue-500 mb-1">{busy}</p>}
            {err && <p className="text-xs text-red-600 mb-1">{err}</p>}
            <pre className="flex-1 overflow-auto text-xs whitespace-pre-wrap bg-neutral-50 dark:bg-neutral-950 rounded p-3 border border-neutral-200 dark:border-neutral-800">
              {sel.md || "(此会议还没有 .md;可能正在后台转写,稍后刷新)"}
            </pre>
          </>
        )}
        {err && !sel && <p className="text-xs text-red-600">{err}</p>}
      </section>
    </div>
  );
}
