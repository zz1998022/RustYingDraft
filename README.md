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
6. 将 `bundle.json + timeline.json` 项目包导入为当前机器可打开的剪映草稿
7. 作为 CLI 被后端或桌面应用调用
8. CLI 同时支持面向人工终端的文本输出和面向后端调用的 JSON 输出
9. VOD 和 bundle 导入过程都可输出进度事件，便于后端实时读取状态
10. CLI 支持从 GitHub Releases 自更新，便于用户侧持续升级
11. 仓库内置 GitHub Actions 多平台构建工作流与统一启动脚本，便于快速打包部署

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
- `crates/jy_bundle`
  负责项目包导入、zip 解包、素材解析与本机绝对路径落地
- `crates/jy_cli`
  对外暴露命令行能力
- `app/`
  Tauri 2.0 桌面端壳子，面向普通剪辑师做本地导入

## 典型用途

### 1. 后端直接生成草稿

你可以先在后端把素材、时间轴、字幕和样式整理成 manifest，再调用 `jy_cli generate` 生成草稿。

### 2. 阿里 VOD 时间轴转剪映草稿

如果你已经有阿里云 VOD 的时间轴 JSON，YingDraft 可以把它转换为本地可打开的剪映草稿，并自动下载远程素材。

### 3. 模板替换

如果你已经准备好了剪映模板草稿，YingDraft 可以按素材名、按片段位置或按文本内容进行替换。

### 4. 本地素材落地工具

如果你的最终交付对象是普通用户，可以把 YingDraft 作为本地导入器的内核，在用户机器上把素材路径重写为绝对路径，再生成最终草稿。

### 5. 桌面导入器

仓库现在已经起了第一版 `app/` Tauri 2.0 壳子：

- 前端目录：`app/src`
- Tauri Rust 入口：`app/src-tauri`
- 共享导入内核：`crates/jy_bundle`

当前桌面端 MVP 已打通：

1. 选择 `.zip` 项目包、项目目录或 `bundle.json`
2. 自动检测常见剪映草稿箱目录
3. 调用共享 Rust 导入内核生成最终草稿

## 草稿兼容性

当前 `jy_draft` 在写草稿时做了两层兼容处理，`generate`、`generate-demo`、`import-bundle`、`vod-json-to-draft` 都会自动吃到：

1. 同时写出 `draft_content.json` 和 `draft_info.json`
   这两份时间线文件内容保持一致，兼容不同版本剪映的草稿入口读取方式。
2. 自动把本地视频、音频素材复制到草稿目录下的 `_assets/`
   草稿 JSON 会改为引用这些本地化后的路径，减少 mac 上因为工作区权限或外部路径不可读导致的素材丢失问题。

这套兼容逻辑当前已经在本机的 mac 高版本剪映上通过实际生成草稿验证。

## 项目包导入

`import-bundle` 是为“本地导入器 / Companion App”准备的第一版跨平台导入内核。

它解决的核心问题是：

1. 服务端只生成项目包，不提前写死用户机器上的素材绝对路径
2. CLI 在用户机器上解析 `bundle.json + timeline.json`
3. 把素材引用解析成当前机器可用的本地绝对路径
4. 最终调用 `write_draft()` 生成剪映草稿

当前命令支持两种项目包：

1. `timeline_package`
   - 输入 `timeline.json`
   - 在本机重新生成草稿
2. `draft_package`
   - 输入现成 `draft/`
   - 在本机重写素材绝对路径

其中 `draft_package` 更适合“后端先用 VOD JSON 生成草稿，再把草稿连同素材一起分发给用户”的场景。

推荐的最小项目包结构：

```text
project_bundle/
  bundle.json
  timeline.json
  assets/
    video/
    audio/
    image/
```

## 快速开始

### 一条命令安装 CLI

macOS / Linux 用户可以直接执行：

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/zz1998022/RustYingDraft/main/scripts/install.sh)"
```

安装后可使用：

```bash
jy --help
```

后续升级可以直接执行：

```bash
jy update
```

如果要更新到指定版本：

```bash
jy update --version v0.1.0
```

默认安装到：

```text
~/.local/opt/YingDraft
```

如果要指定版本或安装目录：

```bash
YINGDRAFT_VERSION=v0.1.0 YINGDRAFT_INSTALL_DIR="$HOME/.local/opt/YingDraft" \
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/zz1998022/RustYingDraft/main/scripts/install.sh)"
```

这个安装方式依赖 GitHub Releases 中的发布包资产。发布 Release 有两种方式：

```bash
git tag v0.1.0
git push origin v0.1.0
```

也可以在 GitHub Actions 里手动运行 `Build Release Bundles`，填写 `release_version`，例如 `v0.1.0`。

Release 会包含这些资产：

- `yingdraft-windows-x64.tar.gz`
- `yingdraft-linux-x64.tar.gz`
- `yingdraft-macos-x64.tar.gz`
- `yingdraft-macos-arm64.tar.gz`

说明：普通 push 到 `main` 只会构建 Actions Artifacts，不会创建 GitHub Release。只有推送 `v*` tag，或手动运行 workflow 并填写 `release_version`，才会发布 Release。

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

如果你准备把 CLI 给 Java / Go / Node 后端调用，也可以直接使用 JSON 输出模式：

```powershell
cargo run -p jy_cli -- --output-format json --help
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

如果运行在阿里云同地域服务器上，可以加 `--use-internal-url`，让远程素材下载优先走 OSS 内网 Endpoint，降低公网流量成本。

### 导入项目包并生成草稿

```powershell
cargo run -p jy_cli -- import-bundle `
  --source ./project_bundle.zip `
  --output ./draft_out `
  --name my_imported_draft
```

`--source` 可以是项目目录、`.zip` 项目包，或者直接指向 `bundle.json`。

### 启动桌面导入器

```powershell
cd app
npm install
npm run tauri dev
```

Rust 侧也可以单独先检查：

```powershell
cargo check -p yingdraft_companion
```

### 面向后端的 JSON 输出

所有命令都支持：

```powershell
cargo run -p jy_cli -- --output-format json <subcommand> ...
```

例如：

```powershell
cargo run -p jy_cli -- --output-format json generate --project ./project.json --output ./draft_out
```

在这个模式下：

- 成功会输出单行 JSON 结果
- 失败会输出单行 JSON 错误
- `vod-json-to-draft` 会持续输出 JSON 进度事件，方便后端实时读取下载状态

## 部署建议

当前最适合的部署形态是：

- 服务端部署 `jy_cli`
- 同时部署 `ffprobe`
- 由 Java 等后端通过命令行调用

如果你的目标是“让用户在自己的电脑上打开草稿”，建议采用：

1. 服务端生成 timeline / manifest / 项目包
2. 本地工具在用户机器上重写绝对路径
3. 本地工具生成最终草稿

如果你需要快速打包 Linux / macOS / Windows 产物，仓库里已经提供：

- `.github/workflows/build-release-bundles.yml`
  - 在 GitHub Actions 中生成 CLI 多平台发布包 artifact，并可在 tag / 手动版本发布时上传到 Release
- `.github/workflows/build-tauri-app.yml`
  - 在 GitHub Actions 中生成 Tauri 桌面端多平台安装包 artifact，并可在 tag / 手动版本发布时上传到 Release
- `scripts/run-jy.ps1`
- `scripts/run-jy.sh`
  - 统一从发布包目录启动 CLI
- `scripts/check-env.ps1`
- `scripts/check-env.sh`
  - 快速检查 `jy_cli` 和 `ffprobe` 是否就绪
- `scripts/install-jy.ps1`
- `scripts/install-jy.sh`
  - 从发布包一键安装到当前用户目录

## 文档

- [使用文档.md](./docs/使用文档.md)
- [部署文档.md](./docs/部署文档.md)
- [Bundle规范.md](./docs/Bundle规范.md)
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
