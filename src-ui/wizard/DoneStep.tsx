export default function DoneStep() {
  return (
    <div className="flex flex-col items-center justify-center text-center gap-3 h-full">
      <div className="text-4xl text-emerald-600">✓</div>
      <h2 className="text-lg font-semibold">一切就绪</h2>
      <p className="max-w-md text-sm text-neutral-500 dark:text-neutral-400">
        按住 <b>左 Win</b> 说话即可输入。ByType 将常驻托盘后台运行,可从托盘打开设置。点「完成」开始。
      </p>
    </div>
  );
}
