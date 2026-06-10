import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cancelDownload, downloadModel, importModel, onDlProgress } from "./api";
import { getConfig } from "../settings/api";
import type { DlProgress } from "./types";

export default function DownloadStep({
  modelReady,
  onReady,
}: {
  modelReady: boolean;
  onReady: () => void;
}) {
  const [phase, setPhase] = useState<"idle" | "downloading">("idle");
  const [prog, setProg] = useState<DlProgress | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [modelUrl, setModelUrl] = useState("");
  const [tokensUrl, setTokensUrl] = useState("");
  const unlisten = useRef<(() => void) | null>(null);

  useEffect(() => {
    onDlProgress(setProg).then((u) => (unlisten.current = u));
    return () => unlisten.current?.();
  }, []);

  useEffect(() => {
    // 预填下载源(config 不存在时后端返回内置 hf-mirror 默认)。
    getConfig()
      .then((r) => {
        setModelUrl(r.config.model.model_url);
        setTokensUrl(r.config.model.tokens_url);
      })
      .catch(() => {});
  }, []);

  const start = async () => {
    if (!modelUrl.trim() || !tokensUrl.trim()) {
      setErr("下载地址不能为空");
      return;
    }
    setErr(null);
    setPhase("downloading");
    setProg(null);
    try {
      await downloadModel(modelUrl, tokensUrl);
      onReady();
    } catch (e) {
      const msg = String(e);
      // 用户主动取消不算失败,不显示红色错误(进度条消失、回到「开始下载」即可)
      setErr(msg.includes("已取消") ? null : msg);
    } finally {
      setPhase("idle");
    }
  };

  const doImport = async () => {
    setErr(null);
    const m = await open({ title: "选择 model.onnx(int8)", filters: [{ name: "ONNX", extensions: ["onnx"] }] });
    if (typeof m !== "string") return;
    const t = await open({ title: "选择 tokens.txt", filters: [{ name: "Text", extensions: ["txt"] }] });
    if (typeof t !== "string") return;
    try {
      await importModel(m, t);
      onReady();
    } catch (e) {
      setErr(String(e));
    }
  };

  const pct = prog && prog.total > 0 ? Math.round((prog.received / prog.total) * 100) : 0;
  const mb = (n: number) => (n / 1024 / 1024).toFixed(1);
  const inputCls =
    "border border-neutral-300 dark:border-neutral-700 dark:bg-neutral-800 rounded-md px-2 py-1 text-xs w-full disabled:opacity-50";
  const downloading = phase === "downloading";

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold">下载语音识别模型</h2>
      {modelReady ? (
        <p className="text-sm text-emerald-600">✓ 模型已就绪,点「下一步」继续。</p>
      ) : (
        <>
          <p className="text-xs text-neutral-400">
            SenseVoice int8 · 约 228MB · 默认源 hf-mirror.com(下不动可改下方地址重试,或用本地导入)
          </p>
          <label className="text-xs text-neutral-500 flex flex-col gap-1">
            模型地址
            <input
              className={inputCls}
              value={modelUrl}
              disabled={downloading}
              placeholder="https://.../model.int8.onnx"
              onChange={(e) => setModelUrl(e.target.value)}
            />
          </label>
          <label className="text-xs text-neutral-500 flex flex-col gap-1">
            词表地址
            <input
              className={inputCls}
              value={tokensUrl}
              disabled={downloading}
              placeholder="https://.../tokens.txt"
              onChange={(e) => setTokensUrl(e.target.value)}
            />
          </label>
          {downloading && (
            <>
              <div className="h-2 bg-neutral-200 dark:bg-neutral-700 rounded-full overflow-hidden">
                <div className="h-full bg-blue-500" style={{ width: `${pct}%` }} />
              </div>
              <div className="flex justify-between text-xs text-neutral-500">
                <span>
                  {prog?.file === "model" ? "模型" : "词表"} · {prog ? mb(prog.received) : "0"} /{" "}
                  {prog && prog.total ? mb(prog.total) : "?"} MB（{pct}%）
                </span>
                <button onClick={() => cancelDownload()} className="text-blue-600">
                  取消
                </button>
              </div>
            </>
          )}
          {!downloading && (
            <button
              onClick={start}
              className="self-start px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600"
            >
              开始下载
            </button>
          )}
          {err && <p className="text-xs text-red-600">下载失败:{err}</p>}
          <div className="border-t border-dashed border-neutral-200 dark:border-neutral-700 pt-3 text-xs text-neutral-400">
            下载不动?
            <button onClick={doImport} className="text-blue-600 hover:underline">
              改用本地文件导入
            </button>
            (选已下好的 model.onnx + tokens.txt,校验后放入)。
          </div>
        </>
      )}
    </div>
  );
}
