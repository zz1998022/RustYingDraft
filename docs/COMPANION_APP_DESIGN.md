# Companion App 设计方案

## 1. 背景

当前主产品是 Web，短期内不考虑整体迁移到 Tauri。

但剪映草稿有一个绕不过去的限制：

- `draft_content.json` 里的素材路径必须是**用户本机上的绝对路径**
- 服务端生成的草稿无法直接保证在用户机器上可用

因此需要一个独立的本地小工具，完成“最后一公里”的草稿落地。

目标：

1. 支持 Windows 和 macOS
2. 图形化
3. 用户操作简单
4. 不影响现有 Web 主产品上线
5. 能复用 `YingDraft` 现有 Rust 能力

## 2. 方案定位

推荐方案：

**Web 主产品 + 独立 Companion App**

职责划分：

- Web 主产品负责：
  - 编辑时间轴
  - 组织素材
  - 生成项目包
  - 提供下载入口

- Companion App 负责：
  - 选择项目包
  - 下载/整理素材到本地
  - 把相对路径或资源引用转换成本机绝对路径
  - 生成最终剪映草稿
  - 可选复制到剪映草稿目录

## 3. 技术选型

推荐：

- 桌面壳：`Tauri`
- 前端：`Vue + Vite`
- 本地逻辑：`Rust`
- 草稿内核：复用当前 `YingDraft`

为什么是这个组合：

1. 你现有前端栈可复用
2. Rust 草稿生成逻辑已经在做
3. Tauri 适合做小体量跨平台文件工具
4. 比 Electron 更轻
5. 比纯 Web 更适合处理本地文件系统和绝对路径

注意：

这里推荐的是**单独做一个 Tauri companion app**，不是把整个主项目迁移到 Tauri。

## 4. 产品目标

第一版 Companion App 的核心目标只有一个：

**把项目包安全地落地为用户本机可打开的剪映草稿**

第一版不追求：

- 复杂项目管理
- 在线编辑
- 自动导出
- 云端同步
- 多账户系统

## 5. 用户流程

建议的最小用户流程：

1. 用户在 Web 端生成项目包
2. 用户下载项目包到本地
3. 用户打开 Companion App
4. 用户选择项目包
5. 用户选择输出目录
6. 用户确认或选择剪映草稿目录
7. 点击“生成草稿”
8. 成功后：
   - 打开草稿目录
   - 或提示用户去剪映中查看

## 6. UI 设计

第一版建议只做 4 个区域：

### 6.1 首页

显示：

- 一个“选择项目包”按钮
- 一个“最近使用项目”列表
- 一个“设置”入口

### 6.2 导入页

显示：

- 项目包路径
- 项目名
- 资源数量
- 预计占用空间
- 输出目录选择器
- 剪映草稿目录选择器
- “开始生成”按钮

### 6.3 进度页

显示：

- 正在校验项目包
- 正在下载素材
- 正在整理素材
- 正在生成草稿
- 正在复制到剪映目录

### 6.4 结果页

显示：

- 成功 / 失败
- 草稿生成目录
- 素材目录
- “打开目录”
- “复制日志”

## 7. 项目包格式

建议不要直接把“最终草稿”作为唯一真相，而是定义一个**项目包格式**。

推荐目录结构：

```text
project_bundle/
  bundle.json
  timeline.json
  assets/
  draft/
```

### 7.1 `bundle.json`

用途：

- 记录项目包元信息
- 校验版本
- 声明资源位置
- 描述导入策略

建议字段：

```json
{
  "bundle_version": 1,
  "project_id": "proj_xxx",
  "project_name": "短剧解说示例",
  "created_at": "2026-04-22T12:00:00Z",
  "generator": {
    "name": "web-backend",
    "version": "1.0.0"
  },
  "timeline_file": "timeline.json",
  "assets_dir": "assets",
  "draft_mode": "generate_from_timeline"
}
```

### 7.2 `timeline.json`

用途：

- 作为项目包内的统一时间轴描述
- Companion App 最终将其转换为 `YingDraft` 的 `Project`

建议不要直接复用完整 manifest，而是使用一份更友好的上层 schema。

理由：

1. 后端更容易生成
2. 前端也更容易消费
3. Companion App 可以在本地补绝对路径、默认值和平台差异

### 7.3 `assets/`

两种模式都支持：

#### 模式 A：素材已包含

```text
assets/
  video/
  audio/
  image/
```

优点：

- 离线可用
- 导入更稳

缺点：

- 包会更大

#### 模式 B：只带资源清单

由 `timeline.json` 中的 URL 告诉 Companion App 去下载。

优点：

- 包小

缺点：

- 依赖网络

建议：

第一版优先支持 **A 和 B 同时兼容**：

- 如果包内有素材，就直接用
- 如果没有素材但有 URL，就在线下载

## 8. 推荐的 Timeline JSON 形态

建议让 Web / 后端生成一份**上层时间轴协议**，而不是直接生成 `generate` 命令用的完整 manifest。

推荐结构：

```json
{
  "project": {
    "id": "proj_001",
    "name": "短剧解说示例"
  },
  "canvas": {
    "width": 1920,
    "height": 1080,
    "fps": 30
  },
  "assets": [
    {
      "id": "video_main",
      "kind": "video",
      "source": {
        "type": "bundle_path",
        "path": "assets/video/main.mp4"
      }
    },
    {
      "id": "bgm_1",
      "kind": "audio",
      "source": {
        "type": "url",
        "url": "https://example.com/bgm.wav"
      }
    }
  ],
  "tracks": [
    {
      "id": "track_video_main",
      "kind": "video",
      "name": "main_video",
      "clips": [
        {
          "id": "clip_1",
          "type": "video",
          "asset_id": "video_main",
          "timeline_in": 0,
          "timeline_out": 5000000,
          "source_in": 0,
          "source_out": 5000000,
          "transform": {
            "x": 0.5,
            "y": 0.5,
            "scale_x": 1.0,
            "scale_y": 1.0,
            "opacity": 1.0
          }
        }
      ]
    }
  ]
}
```

然后由 Companion App 在本地把它转换成 `YingDraft` 的底层 manifest 或直接转换成 `Project`。

## 9. 本地生成流程

Companion App 建议按下面顺序执行：

1. 读取 `bundle.json`
2. 校验 `bundle_version`
3. 解包到工作目录
4. 读取 `timeline.json`
5. 处理资源
   - bundle 内已有素材：直接定位
   - 远程 URL：下载到工作目录
6. 建立 `asset_id -> 本地绝对路径` 映射
7. 将 timeline 转换成 `Project`
8. 调用 `YingDraft` 生成草稿
9. 复制或输出到用户选择的目录
10. 可选自动打开草稿目录

## 10. 工作目录策略

建议本地工具维护一个统一工作目录，例如：

```text
<AppData>/YourApp/projects/<project_id>/
```

在这个目录下放：

```text
projects/<project_id>/
  bundle/
  assets/
  draft/
  logs/
```

好处：

1. 绝对路径稳定
2. 后续可以重复打开同一个项目
3. 容易排查问题
4. 方便做缓存和增量更新

## 11. 草稿输出策略

建议支持两种输出方式：

### 11.1 输出到用户选择目录

适合保守模式。

优点：

- 不猜测用户的剪映目录
- 权限问题更少

### 11.2 复制到剪映草稿目录

适合提升体验。

建议：

- 自动尝试检测常见剪映草稿目录
- 但永远允许用户手动改

不要把“自动检测路径”做成唯一方案。

## 12. 剪映目录策略

因为不同平台和不同安装方式下目录可能变化，建议策略是：

1. 程序启动时尝试检测常见路径
2. 检测成功则作为默认值
3. 始终允许用户手工选择
4. 记录上次成功路径

这样不用把产品绑死在某一个路径上。

## 13. Rust 模块划分建议

推荐新增一个 companion app workspace 或目录，分成以下层次：

### 13.1 `bundle`

职责：

- 解析 `bundle.json`
- 校验版本
- 解包

### 13.2 `timeline_import`

职责：

- 解析上层 `timeline.json`
- 做版本兼容

### 13.3 `asset_resolver`

职责：

- 处理 bundle 内素材
- 下载 URL 素材
- 建立 `asset_id -> 本地绝对路径`

### 13.4 `project_mapper`

职责：

- 将上层 timeline 映射成 `jy_schema::Project`

### 13.5 `draft_output`

职责：

- 调用 `jy_draft::writer::write_draft`
- 拷贝到目标目录

### 13.6 `desktop_api`

职责：

- Tauri command
- 文件选择器
- 系统路径检测

## 14. Tauri 前后端通信

建议只暴露少量稳定 command：

### 14.1 `pick_bundle_file`

用途：

- 选择项目包

### 14.2 `pick_output_dir`

用途：

- 选择输出目录

### 14.3 `detect_jianying_dir`

用途：

- 自动检测草稿目录

### 14.4 `import_bundle`

输入：

- 项目包路径
- 输出目录
- 剪映目录

输出：

- 成功 / 失败
- 生成目录
- 日志

## 15. 最小 MVP 范围

建议第一版只支持：

1. 选择项目包
2. 选择输出目录
3. 解析上层 timeline
4. 处理本地素材和 URL 素材
5. 生成视频、音频、文本、图片四种片段
6. 输出剪映草稿

先不做：

- 自动打开剪映
- 自动导出
- 用户登录
- 项目云同步
- 素材在线编辑

## 16. 错误处理建议

UI 层建议把错误分成这几类：

### 16.1 项目包错误

- 包损坏
- 版本不支持
- 缺少 `bundle.json`

### 16.2 资源错误

- 下载失败
- 资源路径不存在
- 文件格式不支持

### 16.3 草稿错误

- 生成失败
- 输出目录不可写
- 目标草稿目录冲突

### 16.4 平台错误

- 无法检测剪映目录
- 用户未授权访问目标目录

## 17. 数据兼容策略

建议所有对外文件都带版本号：

- `bundle.json`
- `timeline.json`

例如：

```json
{
  "version": 1
}
```

这样以后你改字段时，可以在本地工具里做兼容转换。

## 18. 为什么这是当前最佳折中

你现在的约束是：

- Web 项目快上线
- 不想整体迁移到 Tauri
- 草稿必须依赖绝对路径
- 还要跨平台和图形化

那最合理的解法就是：

**保持 Web 不动，单独补一个跨平台图形化本地 Companion App。**

这样你能同时得到：

1. 现有产品节奏不受影响
2. 用户能在 Windows / macOS 上用
3. 绝对路径问题在本地解决
4. Rust 草稿核心可复用

## 19. 明天实现时的优先级建议

如果明天就开始做，我建议按这个顺序：

1. 先定义 `bundle.json + timeline.json`
2. 做 Rust 命令行原型，不做 GUI
3. 验证“包 -> 本地素材 -> 草稿”闭环
4. 再把这个 CLI 挂到 Tauri GUI 上

原因很简单：

先跑通数据链路，再包一层 UI，速度最快，也最稳。

## 20. 针对“素材绝对路径落地工具”的补充

如果你现在要落地的是一个更聚焦的小工具，也就是：

- 随项目素材一起分发
- 负责把素材变成本机绝对路径
- 让用户图形化选择剪映草稿箱位置

那建议优先看：

- [素材落地工具设计.md](./素材落地工具设计.md)

那份文档比当前这份更偏：

- 最小产品目标
- 图形化交互
- 草稿箱目录选择
- 本地目录策略
- 绝对路径重写流程
