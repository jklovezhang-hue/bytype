# ByType G4 — 设置界面 设计文档

日期:2026-06-10
状态:已批准设计,待写实现计划

## 背景与目标

把主窗口(目前是 G1 占位页)做成**完整设置界面**:读写 `config.toml`、配置 LLM 并一键测试连通、
管理热键/词库/应用风格/各开关,外加帮助与关于页。目标是用户不再需要手动编辑 config.toml。

## 已定决策(经 brainstorming 确认)

| 决策 | 选择 |
|------|------|
| 保存生效方式 | **保存并重启**:写回 config.toml 后自动重启 ByType(约 2-4 秒)。不做引擎热重启/部分热生效 |
| 整体布局 | **左侧导航 + 分页**,共 7 页:通用 / 热键 / LLM 整理 / 词库 / 应用风格 / 帮助 / 关于 |
| G4 额外范围 | 开机自启开关(autostart 插件)+ LLM 高级提示词编辑;**不做**自定义提示音路径 UI |
| 热键编辑 | 8 个支持键的下拉框(非按键捕获),三键互不相同,冲突标红禁存 |
| 词库编辑 | 标签(chips)输入:回车添加、× 删除 |
| 应用风格编辑 | 行表格:匹配串 + 风格语句 + 删除钮,「添加规则」按钮 |
| 测试按钮 | 用**表单当前值**(无需先保存)发一条固定测试请求,显示延迟 + 回复 / 错误 |
| TOML 写回 | `toml` crate 整文件重写。**丢失手写注释**,值全部保留;注释参考 config.example.toml |
| 关于页署名 | © 2026 **Yong Zhang**;联系方式 **jklover2025@outlook.com** |

## 架构

### 核心 crate(src/config.rs、src/corrector.rs)

- 所有配置结构体补 `Serialize`(+`PartialEq` 便于测试):`Config` `HotkeyConfig` `AsrConfig`
  `LlmConfig` `InjectConfig` `AppStyle` `OverlayConfig` `SoundConfig`。
  `AppStyle.match_` 的 `#[serde(rename = "match")]` 对序列化/反序列化同时生效。
- 新增 `Config::load_raw() -> anyhow::Result<(Config, PathBuf)>`:查找并解析 config.toml,
  **路径字段原样保留**(不做相对→绝对解析),返回配置 + 配置文件路径。设置界面用它,
  保证 `./models/sensevoice` 这类相对路径不被改写。
- `load_resolved()` 重构为 `load_raw()` + 路径解析,行为不变(现有测试不动)。
- 新增 `Config::save_to(&self, path: &Path) -> anyhow::Result<()>`:`toml::to_string_pretty`
  整文件写回。UI 未暴露的字段(`inject.mode`、`asr.model_dir`、`sound.start_sound/end_sound`)
  随完整结构体往返,**原值保留**。
- corrector.rs 新增 `pub fn test_connection(cfg: &LlmConfig) -> anyhow::Result<(u64, String)>`:
  返回(耗时 ms, 回复文本)。用传入 cfg 的 base_url/api_key/model/timeout_secs 发一次
  chat/completions(系统提示词固定:`你是连接测试助手,请只回复:你好,ByType!`,用户消息
  `ping`,temperature 0),**不受** `enabled` 与 `skip_if_shorter_than` 影响。复用现有请求代码。

### Tauri 命令(src-tauri 新模块 src/settings.rs)

| 命令 | 行为 |
|------|------|
| `get_config` | `load_raw()` → `{ config, path: Option<String>, error: Option<String> }`。找不到文件:默认值 + path=None;文件存在但解析失败:默认值 + path + error=信息 |
| `save_config(config)` | 后端自行重新查找 config.toml 路径并写回;找不到则写到**程序目录**的 config.toml。错误返回字符串 |
| `test_llm(llm)` | `async` 命令,`spawn_blocking` 调 `corrector::test_connection`,返回 `{ latency_ms, reply }` 或错误字符串 |
| `restart_app` | `app.restart()`。前端在 save_config **成功后**才调它;保存失败不重启 |
| `open_config_dir` | 用资源管理器打开 config.toml 所在目录(找不到则打开程序目录) |

- 开机自启:官方 `tauri-plugin-autostart` 插件。前端用 `@tauri-apps/plugin-autostart` 的
  `enable/disable/isEnabled`,**切换立即生效**(注册表 Run 键),不进 config.toml、不参与脏检查。
- 自定义命令无需 capability;autostart 插件权限要加进 `capabilities/default.json`(main 窗口)。

### 前端(src-ui/)

```
src-ui/
  App.tsx                  设置壳:加载配置、侧边导航、页面切换(useState,不引路由库)、
                           脏检查(JSON.stringify 与加载快照对比)、底部保存栏、错误横幅
  settings/
    types.ts               Config 的 TS 接口,字段名 snake_case 与 serde JSON 完全一致(零映射)
    api.ts                 类型化 invoke 封装:getConfig/saveConfig/testLlm/restartApp/openConfigDir
    consts.ts              KEY_OPTIONS(8 键:LWin=左 Win …)、LANG_OPTIONS(auto/zh/en/yue/ja/ko)、
                           MODE_OPTIONS(clean=忠实清理 / polish=智能整理 / summary=要点提炼)
    widgets.tsx             共享小件:Toggle、Row(标签+说明+控件)、Section、TextInput、
                           NumberInput、Select、Collapsible
    changelog.ts           版本说明静态数据:[{ version, date, items[] }],首条 v0.1.0
    GeneralPage.tsx  HotkeyPage.tsx  LlmPage.tsx  VocabPage.tsx
    AppStylePage.tsx HelpPage.tsx    AboutPage.tsx
```

## 页面明细

1. **通用**:录音浮窗开关(`overlay.enabled`)、提示音开关(`sound.enabled`)、
   开机自启开关(插件,旁注「立即生效」)、识别语言下拉(`asr.language`:自动/中文/英文/粤语/日语/韩语)。
2. **热键**:主键/翻译键/命令键三个下拉(LWin RWin LAlt RAlt LCtrl RCtrl LShift RShift,中文标签);
   任意两键相同 → 冲突项红框 + 红字提示 + 禁用保存按钮。每项带用途说明(按住说话/译成英文/改写选中文字)。
3. **LLM 整理**:启用开关(`llm.enabled`)、接口地址、API Key(password 框 + 👁 明文切换)、模型名、
   整理力度三段选择(mode);**⚡ 测试连接**按钮 + 结果行(✓ 延迟 ms + 回复,或 ✗ 简短原因,回复截断 ~50 字);
   「高级」折叠区:temperature(0–2,步进 0.1)、超时秒数(≥1)、短文本跳过阈值(≥0)、
   整理/翻译/命令三个提示词多行框(placeholder 注明「留空使用内置预设」)。
4. **词库**(`llm.vocabulary`):chips 标签输入,回车添加、× 删除,自动去重去首尾空白。
5. **应用风格**(`app_style`):每行 = 匹配串输入 + 风格语句输入 + 🗑 删除;「+ 添加规则」追加空行;
   保存时过滤掉匹配串为空的行。附说明:前台进程名包含匹配串即生效,取第一条命中。
6. **帮助**(静态,不参与脏检查):
   - 使用说明:热键速查卡,**键名动态读当前表单值**(改了热键说明跟着变);含 Esc/点药丸取消、
     按住不足 0.3 秒丢弃的说明。
   - 常见问题:折叠 Q&A(没出字/测试连接失败/找不到 config.toml/提示音没声音 等)。
   - 版本说明:渲染 changelog.ts(首条 v0.1.0:听写+翻译+语音命令、录音浮窗、提示音、设置界面)。
7. **关于**(静态):应用图标 + ByType + 版本号(`@tauri-apps/api/app` 的 `getVersion()`,
   自动读 tauri.conf.json)、一句话简介、**© 2026 Yong Zhang**、
   联系方式 **jklover2025@outlook.com**(mailto 链接 + 「复制」按钮,navigator.clipboard,
   失败提示手动选中复制)、config.toml 路径展示 + 「打开所在文件夹」按钮(`open_config_dir`;
   path=None 时显示「未找到,保存后将创建于程序目录」)。
   第三方开源致谢留 G6 补充。

### 底部保存栏

- 有未保存改动时出现:`● 有未保存的更改` + 「放弃更改」(回滚到加载快照)+ 「保存并重启」。
- 热键冲突时保存按钮禁用;保存失败时栏内红字显示错误,不重启。
- 保存并重启成功后应用退回托盘静默运行(主窗口默认隐藏)——已知行为,按钮旁小字注明
  「保存后 ByType 将自动重启」。

## 数据流

```
打开设置 → get_config → 表单填充 + 留存快照(autostart 另查 isEnabled)
编辑 → 与快照不同 → 保存栏出现
⚡ 测试连接 → test_llm(表单 llm 节) → ✓ 延迟+回复 / ✗ 原因
保存并重启 → save_config(完整 config) → 写 config.toml → 成功 → restart_app → 新进程按新配置启动
自启开关 → autostart enable/disable(即时,独立于保存)
```

## 校验与错误处理

| 情况 | 处理 |
|------|------|
| 三热键有重复 | 冲突项红框 + 提示,禁用保存 |
| 数字字段越界 | NumberInput 限位(temperature 0–2 / 超时 ≥1 / 阈值 ≥0) |
| 保存失败(权限/磁盘) | 保存栏红字显示错误,不重启 |
| 测试失败 | ✗ + 简短原因(超时 / 401 / 网络),不阻塞其他操作 |
| 找不到 config.toml | 默认值填表 + 黄条「未找到 config.toml,保存时将在程序目录创建」 |
| config.toml 解析失败 | 默认值填表 + 黄条「解析失败:…;保存将整文件覆盖」 |
| autostart 切换失败 | 开关旁红字提示,开关回弹 |

## 依赖与配置变更

- src-tauri/Cargo.toml:`tauri-plugin-autostart = "2"`;lib.rs 注册插件并挂新命令。
- package.json:`@tauri-apps/plugin-autostart`。
- src-tauri/capabilities/default.json:main 窗口加 `autostart:default` 权限。
- 根 crate:无新依赖(serde derive、toml 已有)。
- config.example.toml、窗口尺寸(900×640)不变。

## 测试策略(沿用项目惯例:纯逻辑 TDD,集成真机)

- **TDD(config.rs)**:完整 Config 序列化→反序列化往返值一致(含 app_style 的 `match` 重命名、
  vocabulary、各开关);`save_to` 写临时文件再 `load` 读回一致;序列化保留相对路径原样。
- corrector::test_connection 与 Tauri 命令不写单测(网络/OS 集成,真机验证)。前端无自动化测试(项目惯例)。
- **真机端到端清单**:各项修改→保存并重启→行为变化(换热键生效、关浮窗、关提示音、改 mode);
  测试按钮真实成功 + 故意改错 key 看失败提示;自启开关 → 任务管理器「启动应用」出现/消失;
  打开配置文件夹;帮助页键名跟随表单;关于页版本/署名/邮箱/路径正确;
  热键冲突禁存;放弃更改回滚;无 config.toml 时的黄条与保存创建。

## 非目标(后续或不做)

- 引擎热重启 / 部分配置热生效(保存并重启已覆盖需求)。
- 提示音自定义路径 UI(config.toml 仍可手改)。
- TOML 注释保留(toml_edit)——接受整文件重写。
- 帮助搜索、在线文档、自动生成 changelog、界面多语言。
- `asr.model_dir` 设置 UI 与模型下载 —— 属 G5 首启向导。
- 第三方开源致谢页 —— 属 G6。
