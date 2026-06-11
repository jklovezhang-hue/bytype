import ReactDOM from "react-dom/client";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

type Phase = "idle" | "recording" | "processing" | "done" | "cancelled" | "failed";

/** 秒数格式化:>=1 小时显示 H:MM:SS,否则 MM:SS。 */
function fmtClock(s: number): string {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

function Pill() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [secs, setSecs] = useState(0);
  const [leaving, setLeaving] = useState(false);
  const tick = useRef<number | null>(null);
  const hideT = useRef<number | null>(null);

  // 会议录制中状态(独立于听写;录制期间优先显示会议药丸 + 时分秒计时)。
  const [meeting, setMeeting] = useState(false);
  const [mSecs, setMSecs] = useState(0);
  // 会议转写完成/失败的短暂提示(显示几秒后自动隐藏)。
  const [mNote, setMNote] = useState<string | null>(null);
  const mTick = useRef<number | null>(null);
  const noteT = useRef<number | null>(null);
  // 同步镜像会议状态:bt:state 监听器在 useEffect([]) 里只建一次,闭包会捕获到过期的
  // meeting,故用 ref 让 finishWithFade 等回调读到实时值(会议进行中绝不隐藏共用浮窗)。
  const meetingRef = useRef(false);
  const stopMTick = () => {
    if (mTick.current !== null) {
      clearInterval(mTick.current);
      mTick.current = null;
    }
  };

  const stopTick = () => {
    if (tick.current !== null) {
      clearInterval(tick.current);
      tick.current = null;
    }
  };
  const clearHide = () => {
    if (hideT.current !== null) {
      clearTimeout(hideT.current);
      hideT.current = null;
    }
  };

  // 先停留 holdMs 展示图标,再淡出,最后隐藏窗口。
  // 内层 200ms 淡出计时也存回 hideT,以便淡出途中若新录音到来能被 clearHide 取消。
  const finishWithFade = (holdMs: number) => {
    clearHide();
    hideT.current = window.setTimeout(() => {
      setLeaving(true);
      hideT.current = window.setTimeout(() => {
        // 会议进行中绝不隐藏窗口(听写与会议共用同一浮窗);只复位听写状态,
        // 渲染会自动回落到会议药丸(其计时由 mTick 后台持续,不受影响)。
        if (!meetingRef.current) getCurrentWindow().hide();
        setPhase("idle");
        setLeaving(false);
        setSecs(0);
      }, 200);
    }, holdMs);
  };

  useEffect(() => {
    const un = listen<string>("bt:state", (e) => {
      const s = e.payload as Phase;
      if (s === "recording") {
        stopTick();
        clearHide();
        setLeaving(false);
        setSecs(0);
        const start = Date.now();
        tick.current = window.setInterval(
          () => setSecs(Math.floor((Date.now() - start) / 1000)),
          250
        );
        setPhase("recording");
      } else if (s === "processing") {
        stopTick();
        setLeaving(false);
        setPhase("processing");
      } else if (s === "done") {
        stopTick();
        setPhase("done");
        finishWithFade(500);
      } else if (s === "failed") {
        stopTick();
        setPhase("failed");
        finishWithFade(700);
      } else if (s === "cancelled") {
        stopTick();
        setPhase("cancelled");
        finishWithFade(0);
      }
    });
    // 会议录制中:整场显示一个计时药丸(时分秒),由后端 start/stop 驱动。
    // done/failed:后台转写完成的短暂提示(显示 ~3.2s 后自动隐藏窗口)。
    const unMeeting = listen<string>("bt:meeting", (e) => {
      if (e.payload === "recording") {
        stopMTick();
        setMSecs(0);
        setMNote(null);
        if (noteT.current !== null) clearTimeout(noteT.current);
        const start = Date.now();
        mTick.current = window.setInterval(
          () => setMSecs(Math.floor((Date.now() - start) / 1000)),
          250
        );
        meetingRef.current = true;
        setMeeting(true);
      } else if (e.payload === "done" || e.payload === "failed") {
        stopMTick();
        meetingRef.current = false;
        setMeeting(false);
        setMSecs(0);
        setMNote(e.payload === "done" ? "✅ 会议纪要已就绪" : "✕ 会议处理失败");
        if (noteT.current !== null) clearTimeout(noteT.current);
        noteT.current = window.setTimeout(() => {
          setMNote(null);
          getCurrentWindow().hide();
        }, 3200);
      } else {
        // "stopped"
        stopMTick();
        meetingRef.current = false;
        setMeeting(false);
        setMSecs(0);
      }
    });

    return () => {
      un.then((f) => f());
      unMeeting.then((f) => f());
    };
  }, []);

  const onClick = () => {
    if (meeting) return; // 会议药丸不响应点击取消(结束会议走托盘)
    if (phase !== "recording") return;
    stopTick();
    setPhase("cancelled");
    invoke("cancel_recording").catch(() => {});
    finishWithFade(0);
  };

  // 会议转写完成/失败的短暂提示(会议确已结束,优先)。
  if (mNote) {
    return (
      <div className="pill show" title="会议纪要">
        <span className="time" style={{ fontSize: 13, whiteSpace: "nowrap" }}>{mNote}</span>
      </div>
    );
  }

  // 听写正在进行(录音/处理)时显示听写药丸 —— 即使会议在录制中也临时让位给它,
  // 给出听写反馈;会议计时在后台(mTick)持续不受影响,听写结束后自动回到会议药丸。
  const dictating = phase === "recording" || phase === "processing";

  // 会议录制中且当前无听写活动:显示会议药丸(持续计时)。
  if (meeting && !dictating) {
    return (
      <div className="pill show" title="会议录制中(结束请用托盘菜单)">
        <span className="left">
          <span
            style={{
              width: 10,
              height: 10,
              borderRadius: "50%",
              background: "#ff3b30",
              display: "inline-block",
              animation: "btblink 1.2s ease-in-out infinite",
            }}
          />
        </span>
        <span className="time">{fmtClock(mSecs)}</span>
        <style>{`@keyframes btblink{0%,100%{opacity:1}50%{opacity:.25}}`}</style>
      </div>
    );
  }

  const show = dictating || (phase !== "idle" && !leaving);

  return (
    <div className={`pill ${show ? "show" : ""}`} onClick={onClick} title="点击取消">
      <span className="left">
        {phase === "processing" && <span className="spin" />}
        {phase === "done" && <span className="check">✓</span>}
        {phase === "failed" && <span className="cross">✕</span>}
        {(phase === "recording" || phase === "cancelled") && (
          <span className="time">{secs}</span>
        )}
      </span>
      <span className={`wave ${phase === "recording" ? "" : "static"}`}>
        {Array.from({ length: 8 }).map((_, i) => (
          <i key={i} style={{ animationDelay: `${-i * 0.1}s` }} />
        ))}
      </span>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("overlay-root")!).render(<Pill />);
