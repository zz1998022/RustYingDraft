# Bundle 规范

本文定义本地导入工具读取的 `bundle.json`。

`bundle.json` 只描述打包结构和素材映射，不保存用户电脑上的绝对路径。

## 1. 包类型

`bundle_type` 当前支持四个值：

| 值 | 用途 |
| --- | --- |
| `draft_package` | 包内已经包含剪映草稿，本地导入时只重写素材路径 |
| `timeline_package` | 包内提供时间轴描述，本地导入时重新生成剪映草稿 |
| `simple_timeline_package` | 内部生产用简化时间轴包，仅支持顺序拼视频和字幕 |
| `pipeline_package` | 后端流水线包，复用 concat.txt、分段解说音频和 SRT 字幕生成草稿 |

当前业务优先使用 `draft_package`。

## 2. 目录约定

推荐打包结构：

```text
package_root/
  bundle.json
  draft/
    draft_content.json
    draft_info.json
    draft_meta_info.json
  assets/
    video_0001.mp4
    bgm_0001.mp3
```

桌面导入工具可以和 `bundle.json` 放在同一级目录：

```text
package_root/
  bundle.json
  draft/
  assets/
  YingDraft Companion.exe
```

桌面导入工具启动后会尝试读取同目录下的 `bundle.json`。

## 3. 路径约定

`bundle.json` 中的路径统一使用相对路径。

规则：

- 使用 `/` 作为路径分隔符
- 不写 Windows 反斜杠 `\`
- 不写用户电脑上的绝对路径
- `relative_path` 默认相对于 `assets_dir`
- 如果文件不在 `assets_dir` 下，工具会再尝试按包根目录解析

示例：

```json
{
  "assets_dir": "assets",
  "relative_path": "video_0001.mp4"
}
```

对应文件：

```text
package_root/assets/video_0001.mp4
```

## 4. 顶层字段

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `bundle_version` | `number` | 是 | 当前为 `1` |
| `bundle_type` | `string` | 是 | `draft_package`、`timeline_package`、`simple_timeline_package` 或 `pipeline_package` |
| `project_id` | `string` | 否 | 业务侧项目 ID |
| `project_name` | `string` | 否 | 草稿默认名称 |
| `assets_dir` | `string` | 否 | 素材目录，通常为 `assets` |
| `draft_dir` | `string` | `draft_package` 必填 | 草稿目录，通常为 `draft` |
| `timeline_file` | `string` | `timeline_package` / `simple_timeline_package` 必填 | 时间轴描述文件 |

## 5. `draft_package`

### 5.1 用法

后端已经生成剪映草稿时使用该模式。

导入流程：

1. 复制 `draft_dir` 指向的草稿目录
2. 根据 `assets[]` 找到本机素材文件
3. 按素材名匹配草稿内素材
4. 重写素材路径
5. 输出到用户选择的剪映草稿箱

### 5.2 必填字段

`draft_package` 需要额外提供：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `draft_dir` | `string` | 现有草稿目录 |
| `match_key` | `string` | 当前固定为 `name` |
| `assets` | `array` | 素材映射列表 |

### 5.3 素材映射

`assets[]` 每一项表示一个需要重写路径的素材。

```json
{
  "kind": "video",
  "match_value": "video_0001.mp4",
  "relative_path": "video_0001.mp4"
}
```

字段：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `kind` | `string` | 是 | `video`、`audio`、`image` |
| `match_value` | `string` | 是 | 草稿内用于匹配的素材名 |
| `relative_path` | `string` | 是 | 打包后素材文件路径 |
| `name` | `string` | 否 | 导入后显示的素材名 |

### 5.4 匹配规则

当前只支持：

```json
{
  "match_key": "name"
}
```

匹配字段：

| 素材类型 | 草稿内字段 |
| --- | --- |
| `video` | `materials.videos[].material_name` |
| `image` | `materials.videos[].material_name` |
| `audio` | `materials.audios[].name` |

后端生成草稿时，应保证这些名字稳定。

推荐命名：

```text
video_0001.mp4
video_0002.mp4
bgm_0001.mp3
dubbing_0001.wav
```

不要用草稿内部随机 `id` 作为跨端匹配依据。

### 5.5 示例

```json
{
  "bundle_version": 1,
  "bundle_type": "draft_package",
  "project_id": "vod_quanmingyushou_001",
  "project_name": "全民御兽",
  "draft_dir": "draft",
  "assets_dir": "assets",
  "match_key": "name",
  "assets": [
    {
      "kind": "video",
      "match_value": "video_0001.mp4",
      "relative_path": "video_0001.mp4"
    },
    {
      "kind": "audio",
      "match_value": "bgm_0001.mp3",
      "relative_path": "bgm_0001.mp3"
    }
  ]
}
```

## 6. `timeline_package`

该模式适用于没有现成草稿、只提供时间轴描述的场景。

目录：

```text
package_root/
  bundle.json
  timeline.json
  assets/
```

`bundle.json` 示例：

```json
{
  "bundle_version": 1,
  "bundle_type": "timeline_package",
  "project_id": "proj_001",
  "project_name": "时间轴示例",
  "timeline_file": "timeline.json",
  "assets_dir": "assets"
}
```

`timeline.json` 的结构由导入器单独解析，不在本文展开。

## 7. `simple_timeline_package`

该模式用于内部批量生成普通剪映草稿，不走 VOD 兼容层。第一版只支持：

- 多个视频素材按数组顺序整段拼接
- JSON 字幕
- 全局字幕字号
- 全局字幕位置

目录：

```text
package_root/
  bundle.json
  timeline.json
  assets/
    video_001.mp4
    video_002.mp4
```

`bundle.json` 示例：

```json
{
  "bundle_version": 1,
  "bundle_type": "simple_timeline_package",
  "project_id": "batch_demo_001",
  "project_name": "批量草稿示例",
  "timeline_file": "timeline.json",
  "assets_dir": "assets"
}
```

`timeline.json` 示例：

```json
{
  "canvas": {
    "width": 1920,
    "height": 1080,
    "fps": 30
  },
  "videos": [
    { "path": "video_001.mp4" },
    { "path": "video_002.mp4" }
  ],
  "subtitle_style": {
    "font_size": 8.0,
    "x": 0.5,
    "y": 0.82
  },
  "subtitles": [
    { "start": 0.0, "end": 2.4, "text": "第一句字幕" },
    { "start": 2.4, "end": 5.1, "text": "第二句字幕" }
  ]
}
```

字段说明：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `canvas` | `object` | 否 | 默认使用项目内默认画布；建议生产侧显式传入 |
| `videos` | `array` | 是 | 至少一个视频，按顺序整段拼接 |
| `videos[].path` | `string` | 是 | 相对 `assets_dir` 的视频路径，只允许 `/` 分隔 |
| `videos[].name` | `string` | 否 | 导入后显示的素材名 |
| `subtitle_style.font_size` | `number` | 否 | 字幕字号，默认 `8.0` |
| `subtitle_style.x` | `number` | 否 | 字幕归一化横坐标，`0.5` 为水平居中 |
| `subtitle_style.y` | `number` | 否 | 字幕归一化纵坐标，越大越靠下，默认 `0.82` |
| `subtitles[].start` | `number` | 是 | 字幕开始时间，单位秒 |
| `subtitles[].end` | `number` | 是 | 字幕结束时间，单位秒 |
| `subtitles[].text` | `string` | 是 | 字幕文本 |

校验规则：

- `videos` 不能为空
- 所有视频必须存在于 `assets_dir`
- `videos[].path` 不能是绝对路径，不能包含 `..`，不能使用 Windows 反斜杠
- 字幕 `end` 必须大于 `start`
- 字幕结束时间不能超过拼接后的视频总时长
- 第一版不支持裁剪、空隙、转场、贴纸、音频、多字幕样式、多轨混排

## 8. `pipeline_package`

该模式用于后端已有 ffmpeg 流水线产物的场景。草稿工具不执行 ffmpeg，只读取后端产出的本地素材、SRT 字幕和时间线描述，生成剪映可编辑草稿。

生产新接入推荐使用 `pipeline.tracks` 多轨模式。旧版 `concat_file + subtitle_file + narration_files` 单轨模式继续兼容，但不能和 `pipeline.tracks` 混用。

多轨目录示例：

```text
package_root/
  bundle.json
  assets/
    video/
      main_001.mp4
      main_002.mp4
      overlay_001.mp4
    audio/
      bgm.mp3
    narration/
      001.mp3
      002.mp3
    subtitle/
      cn.srt
      comment.srt
```

多轨 `bundle.json` 示例：

```json
{
  "bundle_version": 1,
  "bundle_type": "pipeline_package",
  "project_id": "batch_demo_001",
  "project_name": "批量草稿_001",
  "assets_dir": "assets",
  "pipeline": {
    "tracks": [
      {
        "kind": "video",
        "name": "main_video",
        "clips": [
          { "path": "video/main_001.mp4", "start": 0.0 },
          { "path": "video/main_002.mp4", "start": 12.5 }
        ]
      },
      {
        "kind": "video",
        "name": "overlay_video",
        "clips": [
          { "path": "video/overlay_001.mp4", "start": 3.0, "volume": 0.0 }
        ]
      },
      {
        "kind": "audio",
        "name": "narration",
        "clips": [
          { "path": "narration/001.mp3", "start": 0.0, "end": 2.4 },
          { "path": "narration/002.mp3", "start": 2.4, "end": 5.1 }
        ]
      },
      {
        "kind": "audio",
        "name": "bgm",
        "clips": [
          { "path": "audio/bgm.mp3", "start": 0.0, "end": 30.0, "volume": 0.35 }
        ]
      },
      {
        "kind": "text",
        "name": "subtitle_cn",
        "subtitle_file": "subtitle/cn.srt",
        "style": { "font_size": 8.0, "x": 0.5, "y": 0.82 }
      },
      {
        "kind": "text",
        "name": "subtitle_comment",
        "subtitle_file": "subtitle/comment.srt",
        "style": { "font_size": 6.0, "x": 0.5, "y": 0.72 }
      }
    ]
  }
}
```

字段说明：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `pipeline.tracks` | `array` | 新模式必填 | 轨道列表 |
| `pipeline.tracks[].kind` | `string` | 是 | `video`、`audio` 或 `text` |
| `pipeline.tracks[].name` | `string` | 是 | 轨道名，必须唯一 |
| `pipeline.tracks[].clips[].path` | `string` | video/audio 必填 | 相对 `assets_dir` 的素材路径 |
| `pipeline.tracks[].clips[].start` | `number` | video/audio 必填 | 片段在时间线上的开始秒数 |
| `pipeline.tracks[].clips[].end` | `number` | audio 必填 | 音频片段结束秒数，视频第一版不支持 `end` |
| `pipeline.tracks[].clips[].volume` | `number` | 否 | 音量，默认 `1.0` |
| `pipeline.tracks[].subtitle_file` | `string` | text 必填 | 相对 `assets_dir` 的 SRT 字幕文件 |
| `pipeline.tracks[].style.font_size` | `number` | 否 | 字幕字号，默认继承全局 `subtitle_style` |
| `pipeline.tracks[].style.x` | `number` | 否 | 字幕归一化横坐标 |
| `pipeline.tracks[].style.y` | `number` | 否 | 字幕归一化纵坐标 |

导入结果：

- 每个 `video` 轨生成一条剪映视频轨，视频整段放到 `start`
- 每个 `audio` 轨生成一条剪映音频轨，真实播放时长为 `min(音频真实时长, end-start)`
- 每个 `text` 轨从一个 SRT 文件生成一条剪映文本轨
- 同轨片段不能重叠，不同轨道之间允许重叠
- 同类型轨道按声明顺序递增层级，后声明的视频轨显示层级更高

校验规则：

- `pipeline.tracks` 不能和 `concat_file`、`subtitle_file`、`narration_files` 混用
- 至少包含一个视频片段
- 轨道名必须唯一
- 视频片段第一版只支持整段素材，不能传 `end`
- 音频片段必须 `end > start`
- 字幕结束时间不能超过所有视频轨的最大结束时间
- 所有路径都相对 `assets_dir`，不能是绝对路径，不能包含 `..`，不能使用 Windows 反斜杠
- 第一版不支持视频裁剪、转场、贴纸、淡入淡出、逐条字幕样式

旧版单轨模式继续支持：

```json
{
  "pipeline": {
    "concat_file": "concat.txt",
    "subtitle_file": "subtitle.srt",
    "narration_files": [
      "narration/001.mp3",
      "narration/002.mp3"
    ]
  },
  "subtitle_style": { "font_size": 8.0, "x": 0.5, "y": 0.82 },
  "audio_style": { "video_volume": 1.0, "narration_volume": 1.0 }
}
```

旧版规则：

- `concat.txt` 至少包含一个 `file '...'` 条目
- `narration_files` 不能为空，数量必须等于 SRT 字幕条数
- 所有路径都相对 `assets_dir`，不能是绝对路径，不能包含 `..`，不能使用 Windows 反斜杠
- SRT 每条字幕必须 `end > start`
- 字幕结束时间不能超过拼接后的视频总时长
- `pipeline.audio_file` 和 `audio_style.audio_volume` 已废弃，传入时报错

## 9. 后端出包流程

使用阿里云 VOD JSON 的场景，推荐流程如下：

1. 调用 `jy_cli vod-json-to-draft`
2. 将生成的草稿目录放到 `package_root/draft`
3. 将素材文件放到 `package_root/assets`
4. 生成 `package_root/bundle.json`
5. 将整个 `package_root` 打包或直接分发

最终目录示例：

```text
package_root/
  bundle.json
  draft/
    draft_content.json
    draft_info.json
    draft_meta_info.json
  assets/
    video_0001.mp4
    bgm_0001.mp3
  YingDraft Companion.exe
```

## 10. 校验清单

出包前建议检查：

- `bundle_version` 为 `1`
- `bundle_type` 为 `draft_package`、`timeline_package`、`simple_timeline_package` 或 `pipeline_package`
- `draft/draft_content.json` 存在
- `draft/draft_info.json` 存在
- `draft/draft_meta_info.json` 存在
- `assets[]` 中的每个 `relative_path` 都能找到文件
- `assets[]` 中的每个 `match_value` 都能在草稿素材列表中匹配到
- 素材名不要重复；如果重复，导入时会报歧义错误
