export default function WelcomeStep() {
  return (
    <div className="flex flex-col items-center justify-center text-center gap-3 h-full">
      <div className="w-14 h-14 rounded-2xl bg-blue-500" />
      <h2 className="text-lg font-semibold">欢迎使用 ByType</h2>
      <p className="max-w-md text-sm text-neutral-500 dark:text-neutral-400">
        按住热键说话,松手即把文字输入到任何应用。首次使用需几步准备:检测运行环境、填写 LLM
        中转站(可选)、下载语音识别模型(约 228MB)。
      </p>
    </div>
  );
}
