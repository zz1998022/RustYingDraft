# YingDraft

YingDraft 是一个用 Rust 编写的剪映草稿生成与编辑工具集。

它的目标不是模拟剪映界面，而是直接生成和修改剪映草稿目录里的核心文件，例如：

- `draft_content.json`
- `draft_info.json`
- `draft_meta_info.json`

你可以把它理解为一个“草稿生成内核”，适合挂在这些场景下面：

- Java / Go / Node 服务端
- 本地 Companion App
- 时间轴转换流水线
- AI 短剧解说与自动剪辑系统

## 当前能力

目前已经具备这些核心能力：

1. 根据结构化 `Project / Track / Clip` 数据生成剪映草稿
2. 读取已有草稿并执行模板替换
3. 将阿里云 VOD 时间轴 JSON 转换为剪映草稿
4. 处理本地视频、音频、图片、字幕等素材
5. 生成兼容 mac / Windows 剪映的草稿入口文件，并自动将素材本地化到草稿目录
6. 作为 CLI 被后端或桌面应用调用

## 项目结构

- `crates/jy_schema`
  定义统一数据模型，例如 `Project`、`Track`、`Clip`、`Transform`、`TextStyle`
- `crates/jy_media`
  负责媒体探测、素材路径规范化、时长和尺寸读取
- `crates/jy_timeline`
  负责把上层时间轴输入组装成 clip 和 track
- `crates/jy_draft`
  负责把 `Project` 转成剪映草稿 JSON 并写入草稿目录
- `crates/jy_template`
  负责模板草稿的素材替换、文本替换和草稿复制
- `crates/jy_cli`
  对外暴露命令行能力

## 典型用途

### 1. 后端直接生成草稿

你可以先在后端把素材、时间轴、字幕和样式整理成 manifest，再调用 `jy_cli generate` 生成草稿。

### 2. 阿里 VOD 时间轴转剪映草稿

如果你已经有阿里云 VOD 的时间轴 JSON，YingDraft 可以把它转换为本地可打开的剪映草稿，并自动下载远程素材。

### 3. 模板替换

如果你已经准备好了剪映模板草稿，YingDraft 可以按素材名、按片段位置或按文本内容进行替换。

### 4. 本地素材落地工具

如果你的最终交付对象是普通用户，可以把 YingDraft 作为本地导入器的内核，在用户机器上把素材路径重写为绝对路径，再生成最终草稿。

## 草稿兼容性

当前 `jy_draft` 在写草稿时做了两层兼容处理，`generate`、`generate-demo`、`vod-json-to-draft` 都会自动吃到：

1. 同时写出 `draft_content.json` 和 `draft_info.json`
   这两份时间线文件内容保持一致，兼容不同版本剪映的草稿入口读取方式。
2. 自动把本地视频、音频素材复制到草稿目录下的 `_assets/`
   草稿 JSON 会改为引用这些本地化后的路径，减少 mac 上因为工作区权限或外部路径不可读导致的素材丢失问题。

这套兼容逻辑当前已经在本机的 mac 高版本剪映上通过实际生成草稿验证。

## 快速开始

### 环境要求

- Rust / Cargo
- `ffprobe`

检查命令：

```powershell
rustc --version
cargo --version
ffprobe -version
```

### 运行测试

```powershell
cd /path/to/YingDraft
cargo test
```

### 查看 CLI 命令

```powershell
cargo run -p jy_cli -- --help
```

## 常用命令

### 初始化一个空 manifest

```powershell
cargo run -p jy_cli -- init --name my_project --width 1920 --height 1080 --fps 30 --output ./project.json
```

### 根据 manifest 生成草稿

```powershell
cargo run -p jy_cli -- generate --project ./project.json --output ./draft_out
```

生成后的 `draft_out` 目录里默认会包含：

- `draft_content.json`
- `draft_info.json`
- `draft_meta_info.json`
- `_assets/`

### 生成本地 demo 草稿

```powershell
cargo run -p jy_cli -- generate-demo --help
```

### 将阿里 VOD JSON 转换为草稿

```powershell
cargo run -p jy_cli -- vod-json-to-draft --help
```

## 部署建议

当前最适合的部署形态是：

- 服务端部署 `jy_cli`
- 同时部署 `ffprobe`
- 由 Java 等后端通过命令行调用

如果你的目标是“让用户在自己的电脑上打开草稿”，建议采用：

1. 服务端生成 timeline / manifest / 项目包
2. 本地工具在用户机器上重写绝对路径
3. 本地工具生成最终草稿

## 文档

- [使用文档.md](./docs/使用文档.md)
- [部署文档.md](./docs/部署文档.md)
- [Manifest规范.md](./docs/Manifest规范.md)
- [COMPANION_APP_DESIGN.md](./docs/COMPANION_APP_DESIGN.md)
- [素材落地工具设计.md](./docs/素材落地工具设计.md)

## 当前状态说明

这个仓库目前更接近“工程化内核”，而不是已经打磨完成的终端产品。

比较适合：

- 继续二次开发
- 接入你自己的后端
- 作为桌面端或本地导入器的核心模块

## 许可证状态

当前仓库**暂未附带开源许可证**。

在许可证问题正式确认之前，请先将它视为：

- 暂不对外授权的代码仓库
- 不建议直接按开源项目方式分发和复用

后续如果许可证策略明确，再补充正式 `LICENSE` 文件。
