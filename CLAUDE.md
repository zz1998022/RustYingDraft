# YingDraft 开发规范

本文件是本仓库的长期协作规范。后续任何自动化助手或开发者在修改本项目时，都必须先阅读并遵守这里的约束。

## 项目定位

YingDraft 是用 Rust 编写的剪映草稿生成与编辑内核。项目目标不是模拟剪映 GUI，而是直接生成、读取、改写剪映草稿目录中的核心文件：

- `draft_content.json`
- `draft_info.json`
- `draft_meta_info.json`

所有功能最终都应收敛到统一的 `Project / Track / Clip / Material` 模型，再由 `jy_draft` 写出剪映草稿。

## 当前架构

Rust workspace 由以下模块组成：

- `crates/jy_schema`
  - 统一领域模型层。
  - 定义 `Project`、`Track`、`Clip`、`Transform`、`TextStyle`、`TimeRange`、素材引用等结构。
  - 不应依赖其他本地业务 crate。
- `crates/jy_media`
  - 媒体探测与素材引用创建层。
  - 负责调用 `ffprobe`、识别视频/音频/图片/GIF、读取时长和尺寸、生成本机绝对路径素材。
- `crates/jy_timeline`
  - 时间轴构建层。
  - 负责轨道创建、片段创建、时间范围推导、轨道去重、类型匹配、同轨重叠校验、工程总时长维护。
- `crates/jy_draft`
  - 剪映草稿转换与落盘层。
  - 负责 `Project -> draft_content.json`，以及写入 `draft_content.json`、`draft_info.json`、`draft_meta_info.json`。
  - 草稿 JSON 字段兼容问题优先在这里排查。
- `crates/jy_template`
  - 模板草稿编辑层。
  - 负责复制模板、替换素材、替换文本、读取已有草稿结构。
- `crates/jy_bundle`
  - 项目包导入层。
  - 负责 `bundle.json + timeline.json`、zip 解包、素材落地、`timeline_package` 和 `draft_package` 导入。
- `crates/jy_cli`
  - 对外命令行编排层。
  - 负责解析外部输入、调用底层 crate、输出文本或 JSON 事件。
  - 不应把草稿 JSON 细节散落到 CLI 中，除非是在做特定输入格式到 `Project` 的转换。
- `app/`
  - Tauri 2 桌面导入器。
  - 应复用 `jy_bundle` 等共享内核，不另写一套导入逻辑。

## 分层规则

1. `jy_schema` 是最低层数据契约，新增能力先确认是否需要扩展 schema。
2. 时间轴拼接、片段合法性、时长推导放在 `jy_timeline`。
3. 剪映 JSON 字段、素材区、轨道区、meta 文件写法放在 `jy_draft`。
4. 外部输入格式解析放在 `jy_cli/src/commands/` 或 `jy_bundle`，解析后必须转换为统一模型。
5. 桌面端只做 UI 和本地调用编排，不复制核心业务规则。
6. 不允许为一个功能绕开现有分层直接拼完整草稿 JSON，除非是在模板导入/兼容旧草稿这类必须保留原始 JSON 的场景。

## 兼容性规则

1. 剪映草稿 JSON 是核心兼容契约，字段增删必须谨慎。
2. `draft_content.json` 和 `draft_info.json` 当前默认保持一致；不要随意破坏这个兼容策略。
3. Windows 下写入草稿 JSON 的素材路径优先使用正斜杠格式，例如 `D:/path/file.mp4`。
4. manifest 或 bundle 中的素材路径必须是生成草稿机器上真实存在的本机路径。
5. 时间单位统一使用微秒，`1s = 1_000_000`。
6. 素材、片段、速度、动画、特效等 ID 默认使用无连字符 UUID，除非为了兼容已有模板必须保留原 ID。
7. 从 `pyJianYingDraft` 迁移行为时，必须用原项目生成的草稿 JSON 对照，不要只按视觉印象猜字段。

## CLI 规则

1. `jy_cli` 必须同时服务人工终端和后端调用。
2. 新命令或改命令时，要保持 `--output-format json` 的结构化输出可用。
3. 长任务必须输出进度事件，尤其是远程素材下载、bundle 导入、批量处理。
4. 错误输出要保留真实错误链，不要吞掉上游 provider、HTTP、文件系统或 ffprobe 的具体错误。
5. 不要轻易破坏已有 CLI 参数名；如果必须改，先保留兼容路径，并更新 README/docs。

## pyJianYingDraft 对齐规则

1. pyJianYingDraft 是本项目早期行为参考源之一。
2. 对齐 demo、动画、转场、文字气泡、花字、背景填充等能力时，要同时检查：
   - `materials.*`
   - `tracks[].segments[]`
   - `extra_material_refs`
   - `target_timerange` / `source_timerange`
   - `clip.transform`
   - `render_index`
3. 官方 demo 对齐基准是 `D:\Workspace\pyJianYingDraft\demo.py` 的行为。
4. 如果原版草稿被剪映打开后重写为非 JSON 或编码内容，应重新生成一个未被剪映触碰的 compare 草稿再对照。

## VOD 转换规则

1. `vod-json-to-draft` 是当前重要业务入口，不要因为 demo、模板或 bundle 改动误伤它。
2. VOD 转换中的远程素材必须先落地到本机，因为剪映不使用 HTTP URL 作为素材路径。
3. `--assets-dir` 语义要保持稳定；VOD 链路默认不再把素材二次复制进草稿 `_assets/`。
4. `--use-internal-url` 只应改写明确的阿里云 OSS 公网 endpoint，不要影响自定义域名、加速域名或已是 internal 的域名。
5. 字幕、GlobalImage、AdaptMode 等映射若要增强，必须用真实 VOD 样例和剪映打开效果验证。

## Bundle 规则

1. `timeline_package` 用于本地重新生成草稿。
2. `draft_package` 用于复制已有草稿并重写素材路径。
3. `draft_package` 的素材替换必须同步覆盖 `draft_content.json`、`draft_info.json`、`template-2.tmp`、`Timelines/*` 下相关快照。
4. bundle 导入输出目录必须为空或不存在，避免覆盖用户草稿。

## 测试与验证

1. 修改某个 crate 后，至少运行对应 crate 的测试，例如：
   - `cargo test -p jy_cli`
   - `cargo test -p jy_bundle`
   - `cargo test -p jy_draft`
2. 修改 CLI 后，至少运行相关命令的 `--help` 或一个最小真实命令。
3. 修改草稿 JSON 输出后，必须检查生成目录中是否包含预期文件，并抽查 `draft_content.json` 的轨道、素材、时长。
4. 修改 demo 对齐逻辑时，要生成 Rust 草稿并和 pyJianYingDraft 原版草稿做 JSON 摘要对照。
5. Windows 环境可能存在 Cargo target 文件锁或 `.git/index.lock` 权限问题；遇到权限问题先确认是否有残留进程，再谨慎处理。

## 修改范围约束

1. 用户指定一个命令、一个模块、一个链路时，不要顺手重构其他模块。
2. VOD、bundle、template、demo 是不同业务边界，除非用户明确要求，不要跨边界改动。
3. 文档、README、规范文件的更新必须反映真实实现，不要写未来计划当作已完成能力。
4. 不要因为测试方便降低兼容性要求。
5. 不要删除旧能力来换取新 demo 通过，除非用户明确同意。

## 代码风格

1. 优先使用现有 crate、结构体和 helper，不新增重复抽象。
2. 复杂行为加简短中文注释，解释“为什么”，不要解释显而易见的语法。
3. 使用 `camino::Utf8Path` / `Utf8PathBuf` 处理对外路径，保持现有风格。
4. 结构化数据用 `serde` 解析和生成，不用手写字符串拼 JSON。
5. 错误处理使用 `anyhow::Context` 或 crate 自有 error 类型补充上下文。
6. 不要引入新依赖，除非确实能减少复杂度或匹配现有架构。

## 提交流程

1. 提交前运行和改动相关的测试。
2. 提交前确认 `git status --short`，只包含本次任务相关文件。
3. commit message 用简洁中文描述行为变化。
4. 用户要求推送时，完成验证、提交并推送到当前远程分支。
