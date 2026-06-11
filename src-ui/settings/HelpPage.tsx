import { CHANGELOG } from "./changelog";
import { keyLabel } from "./consts";
import type { Config } from "./types";
import { Section } from "./widgets";

const FAQS: [string, string][] = [
  [
    "按了热键没出字?",
    "确认目标输入框拥有焦点;以管理员权限运行的程序需要 ByType 也以管理员身份运行;说话需按住至少 0.3 秒,过短视为误触丢弃。",
  ],
  [
    "测试连接失败?",
    "检查接口地址(通常以 /v1 结尾)、API Key 与网络;在「LLM 整理」页点「测试连接」可看到具体原因。",
  ],
  [
    "找不到 config.toml?",
    "在「关于」页可查看配置文件路径;若文件不存在,在设置里点「保存并重启」会自动创建。",
  ],
  [
    "提示音没声音?",
    "确认「通用」页提示音开关已开,且 Windows 音量混合器中 ByType 未被静音。",
  ],
];

export default function HelpPage({ cfg }: { cfg: Config }) {
  const p = keyLabel(cfg.hotkey.primary);
  const t = keyLabel(cfg.hotkey.translate_modifier);
  const m = keyLabel(cfg.hotkey.command_modifier);

  return (
    <div className="flex flex-col gap-6">
      <Section title="使用说明">
        <ul className="text-sm text-neutral-700 dark:text-neutral-300 flex flex-col gap-1.5 list-disc pl-5">
          <li>
            按住 <b>{p}</b> 说话,松手自动识别、整理并输入到当前光标处。
          </li>
          <li>
            按住 <b>{p} + {t}</b> 说话:中英互译——说中文出英文,说英文等其他语言出中文,并自动纠正语法。
          </li>
          <li>
            先选中一段文字,按住 <b>{p} + {m}</b> 说出修改指令(如"改得正式一点"):用结果替换选中文字。
          </li>
          <li>
            不选中文字,直接按住 <b>{p} + {m}</b> 说话:把口述内容去语气词、纠错后<b>总结</b>成简洁文字输出。
          </li>
          <li>
            录音中按 <b>Esc</b> 或点击底部药丸:取消本次录音,不出字。
          </li>
          <li>按住不足 0.3 秒视为误触,自动丢弃。</li>
        </ul>
      </Section>
      <Section title="常见问题">
        {FAQS.map(([q, a]) => (
          <details key={q} className="text-sm">
            <summary className="cursor-pointer text-neutral-800 dark:text-neutral-200">{q}</summary>
            <p className="mt-1.5 text-neutral-500 dark:text-neutral-400 pl-4">{a}</p>
          </details>
        ))}
      </Section>
      <Section title="版本说明">
        {CHANGELOG.map((r) => (
          <div key={r.version} className="text-sm">
            <div className="font-medium text-neutral-800 dark:text-neutral-200">
              v{r.version}
              <span className="text-xs text-neutral-400 ml-2">{r.date}</span>
            </div>
            <ul className="mt-1 list-disc pl-5 text-neutral-600 dark:text-neutral-300">
              {r.items.map((it) => (
                <li key={it}>{it}</li>
              ))}
            </ul>
          </div>
        ))}
      </Section>
    </div>
  );
}
