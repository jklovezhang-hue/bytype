import ReactDOM from "react-dom/client";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

type Phase = "idle" | "recording" | "processing" | "done" | "cancelled" | "failed";

function Pill() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [secs, setSecs] = useState(0);
  const [leaving, setLeaving] = useState(false);
  const tick = useRef<number | null>(null);
  const hideT = useRef<number | null>(null);

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

  // 先停留 holdMs 展示图标,再淡出,最后隐藏窗口
  const finishWithFade = (holdMs: number) => {
    clearHide();
    hideT.current = window.setTimeout(() => {
      setLeaving(true);
      window.setTimeout(() => {
        getCurrentWindow().hide();
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
    return () => {
      un.then((f) => f());
    };
  }, []);

  const onClick = () => {
    if (phase !== "recording") return;
    stopTick();
    setPhase("cancelled");
    invoke("cancel_recording").catch(() => {});
    finishWithFade(0);
  };

  const show = phase !== "idle" && !leaving;

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
