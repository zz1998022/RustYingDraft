# Manifest 规范

## 1. 作用

本文档描述 `jy_cli generate` 命令读取的 manifest JSON 格式。

适用场景：

- 后端先完成素材整理、时间轴编排、字幕对齐
- 再将结果输出为一份 manifest JSON
- 最后由 `YingDraft` 把 manifest JSON 转成剪映草稿

对应命令：

```powershell
cargo run -p jy_cli -- generate --project D:\temp\project.json --output 'D:\JianyingPro Drafts\my_project'
```

对应实现：

- [generate.rs](./crates/jy_cli/src/commands/generate.rs)

## 2. 顶层结构

顶层结构对应 `ProjectManifest`，最终会被收敛为 [Project](./crates/jy_schema/src/project.rs)。

```json
{
  "id": "optional_project_id",
  "name": "project_name",
  "canvas": {
    "width": 1920,
    "height": 1080,
    "fps": 30
  },
  "maintrack_adsorb": true,
  "tracks": [],
  "video_materials": [],
  "audio_materials": [],
  "duration": 0
}
```

### 2.1 顶层字段说明

| 字段 | 类型 | 必填 | 说明 |
|---|---|---:|---|
| `id` | `string` | 否 | 工程 ID。可省略，CLI 会自动生成。 |
| `name` | `string` | 是 | 工程名，同时也会作为草稿名写入剪映。 |
| `canvas` | `object` | 是 | 画布配置。 |
| `maintrack_adsorb` | `boolean` | 否 | 是否启用主轨吸附。默认 `true`。 |
| `tracks` | `array` | 否 | 轨道数组。默认空数组。 |
| `video_materials` | `array` | 否 | 视频/图片素材数组。默认空数组。 |
| `audio_materials` | `array` | 否 | 音频素材数组。默认空数组。 |
| `duration` | `number` | 否 | 工程总时长，单位微秒。省略时会根据所有片段的结束时间自动推导。 |

## 3. 画布配置

画布结构对应 [Canvas](./crates/jy_schema/src/canvas.rs)。

```json
{
  "width": 1920,
  "height": 1080,
  "fps": 30
}
```

### 3.1 字段说明

| 字段 | 类型 | 必填 | 说明 |
|---|---|---:|---|
| `width` | `number` | 是 | 输出宽度，单位像素。 |
| `height` | `number` | 是 | 输出高度，单位像素。 |
| `fps` | `number` | 是 | 帧率。 |

## 4. 时间单位

manifest 内部所有时间字段统一使用：

**微秒（microseconds）**

也就是：

- `1 秒 = 1_000_000`

对应实现常量：

- [SEC](./crates/jy_schema/src/time.rs)

### 4.1 常见示例

| 时长 | 微秒值 |
|---|---:|
| `0.5s` | `500000` |
| `1s` | `1000000` |
| `2.5s` | `2500000` |
| `1min` | `60000000` |

### 4.2 关键时间字段

| 字段 | 位置 | 含义 |
|---|---|---|
| `duration` | 顶层 | 工程总时长 |
| `target_timerange.start` | clip 内 | 片段在时间轴上的开始时间 |
| `target_timerange.duration` | clip 内 | 片段在时间轴上的持续时间 |
| `source_timerange.start` | 音视频 clip 内 | 从素材源文件的哪个位置开始截 |
| `source_timerange.duration` | 音视频 clip 内 | 从素材源文件截多长 |

## 5. 素材定义

manifest 里的素材分成两类：

- `video_materials`
- `audio_materials`

文本不需要提前放到 `materials` 区域，`TextClip` 会在导出阶段自动生成文本素材。

### 5.1 视频/图片素材

对应结构：

- [VideoMaterialRef](./crates/jy_schema/src/material.rs)

```json
{
  "id": "video_main",
  "path": "D:/assets/main.mp4",
  "duration": 255793034,
  "width": 1080,
  "height": 1920,
  "kind": "Video",
  "crop": {
    "upper_left_x": 0.0,
    "upper_left_y": 0.0,
    "upper_right_x": 1.0,
    "upper_right_y": 0.0,
    "lower_left_x": 0.0,
    "lower_left_y": 1.0,
    "lower_right_x": 1.0,
    "lower_right_y": 1.0
  },
  "name": "main.mp4"
}
```

#### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|---|---|---:|---|
| `id` | `string` | 是 | 素材 ID，clip 会通过 `material_id` 引用它。 |
| `path` | `string` | 是 | 本机绝对路径。建议统一使用正斜杠。 |
| `duration` | `number` | 是 | 素材总时长，单位微秒。图片可以使用一个很大的默认值。 |
| `width` | `number` | 是 | 素材宽度。 |
| `height` | `number` | 是 | 素材高度。 |
| `kind` | `string` | 是 | 素材类型，见下方枚举值。 |
| `crop` | `object` | 是 | 裁剪区域。通常不裁剪就传默认值。 |
| `name` | `string` | 是 | 素材名称。 |

#### `kind` 可选值

| 值 | 说明 |
|---|---|
| `Video` | 视频素材 |
| `Photo` | 图片素材 |
| `Audio` | 不建议用于 `video_materials`，仅为 schema 完整性保留 |

### 5.2 音频素材

对应结构：

- [AudioMaterialRef](./crates/jy_schema/src/material.rs)

```json
{
  "id": "audio_bgm",
  "path": "D:/assets/bgm.wav",
  "duration": 255793034,
  "name": "bgm.wav"
}
```

#### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|---|---|---:|---|
| `id` | `string` | 是 | 音频素材 ID。 |
| `path` | `string` | 是 | 本机绝对路径。 |
| `duration` | `number` | 是 | 素材总时长，单位微秒。 |
| `name` | `string` | 是 | 音频素材名称。 |

## 6. 轨道定义

轨道结构对应：

- [Track](./crates/jy_schema/src/track.rs)

```json
{
  "id": "track_video_main",
  "kind": "Video",
  "name": "main_video",
  "render_index": 0,
  "mute": false,
  "clips": []
}
```

### 6.1 字段说明

| 字段 | 类型 | 必填 | 说明 |
|---|---|---:|---|
| `id` | `string` | 是 | 轨道 ID。 |
| `kind` | `string` | 是 | 轨道类型。 |
| `name` | `string` | 是 | 轨道名。建议稳定命名，便于模板替换。 |
| `render_index` | `number` | 是 | 层级索引，越大越靠前。 |
| `mute` | `boolean` | 是 | 轨道是否静音。 |
| `clips` | `array` | 是 | 轨道中的片段数组。 |

### 6.2 `kind` 可选值

| 值 | 说明 |
|---|---|
| `Video` | 视频轨 |
| `Audio` | 音频轨 |
| `Text` | 文本轨 |
| `Effect` | 特效轨 |
| `Filter` | 滤镜轨 |
| `Sticker` | 贴纸轨 |

### 6.3 轨道和片段类型的匹配关系

当前推荐使用：

| 轨道类型 | 可放片段 |
|---|---|
| `Video` | `Video`、`Image` |
| `Audio` | `Audio` |
| `Text` | `Text` |

虽然 schema 里还有 `Effect / Filter / Sticker` 轨，但如果你只是后端拼时间轴，第一版通常只需要 `Video / Audio / Text`。

## 7. Clip 结构

manifest 中的 `clips` 采用 **externally tagged enum** 形式。

也就是说每个元素长这样：

```json
{ "Video": { ... } }
{ "Audio": { ... } }
{ "Text": { ... } }
{ "Image": { ... } }
```

对应 enum：

- [Clip](./crates/jy_schema/src/clip.rs)

### 7.1 Video clip

对应结构：

- [VideoClip](./crates/jy_schema/src/clip.rs)

```json
{
  "Video": {
    "id": "clip_video_1",
    "material_id": "video_main",
    "target_timerange": {
      "start": 0,
      "duration": 5000000
    },
    "source_timerange": {
      "start": 0,
      "duration": 5000000
    },
    "speed": {
      "id": "speed_1",
      "speed": 1.0
    },
    "volume": 1.0,
    "change_pitch": false,
    "transform": {
      "x": 0.5,
      "y": 0.5,
      "scale_x": 1.0,
      "scale_y": 1.0,
      "rotation_deg": 0.0,
      "opacity": 1.0,
      "flip_h": false,
      "flip_v": false,
      "uniform_scale": true
    },
    "keyframes": [],
    "fade": null,
    "effects": [],
    "filters": [],
    "mask": null,
    "transition": null,
    "background_filling": null,
    "animations": null,
    "mix_mode": null
  }
}
```

#### 关键字段说明

| 字段 | 说明 |
|---|---|
| `material_id` | 必须引用 `video_materials[].id` |
| `target_timerange` | 片段在时间轴上的位置 |
| `source_timerange` | 从源素材截取的时间范围 |
| `speed.speed` | 播放速度 |
| `volume` | 视频原声音量 |
| `transform` | 位置、缩放、透明度等 |

### 7.2 Audio clip

对应结构：

- [AudioClip](./crates/jy_schema/src/clip.rs)

```json
{
  "Audio": {
    "id": "clip_audio_1",
    "material_id": "audio_bgm",
    "target_timerange": {
      "start": 0,
      "duration": 5000000
    },
    "source_timerange": {
      "start": 0,
      "duration": 5000000
    },
    "speed": {
      "id": "speed_audio_1",
      "speed": 1.0
    },
    "volume": 0.3,
    "change_pitch": false,
    "keyframes": [],
    "fade": null,
    "effects": []
  }
}
```

### 7.3 Text clip

对应结构：

- [TextClip](./crates/jy_schema/src/clip.rs)

```json
{
  "Text": {
    "id": "clip_text_1",
    "material_id": "text_mat_1",
    "target_timerange": {
      "start": 0,
      "duration": 2000000
    },
    "text": "第一句字幕",
    "font": null,
    "style": {
      "size": 7.2,
      "bold": false,
      "italic": false,
      "underline": false,
      "color": [1.0, 1.0, 1.0],
      "alpha": 1.0,
      "align": "Center",
      "vertical": false,
      "letter_spacing": 0,
      "line_spacing": 0,
      "auto_wrapping": true,
      "max_line_width": 0.82
    },
    "transform": {
      "x": 0.5,
      "y": 0.1,
      "scale_x": 1.0,
      "scale_y": 1.0,
      "rotation_deg": 0.0,
      "opacity": 1.0,
      "flip_h": false,
      "flip_v": false,
      "uniform_scale": true
    },
    "keyframes": [],
    "border": {
      "alpha": 1.0,
      "color": [0.0, 0.0, 0.0],
      "width": 0.08
    },
    "background": null,
    "shadow": {
      "alpha": 0.35,
      "color": [0.0, 0.0, 0.0],
      "diffuse": 18.0,
      "distance": 5.0,
      "angle": -45.0
    },
    "animations": null,
    "bubble": null,
    "effect": null
  }
}
```

#### Text clip 特别说明

1. `material_id` 不需要在顶层 `materials` 里提前声明  
   - `converter` 会根据 `TextClip` 自动生成文字素材
2. `style.align` 是字符串枚举  
   - 可选值：`Left`、`Center`、`Right`
3. `border.width` 是内部值，不是 UI 的 `0~100`
   - 当前内部使用范围大致是 `0.0 ~ 0.2`

### 7.4 Image clip

对应结构：

- [ImageClip](./crates/jy_schema/src/clip.rs)

```json
{
  "Image": {
    "id": "clip_image_1",
    "material_id": "img_watermark",
    "target_timerange": {
      "start": 0,
      "duration": 10000000
    },
    "source_timerange": {
      "start": 0,
      "duration": 10000000
    },
    "speed": {
      "id": "speed_img_1",
      "speed": 1.0
    },
    "transform": {
      "x": 0.86,
      "y": 0.11,
      "scale_x": 0.22,
      "scale_y": 0.22,
      "rotation_deg": 0.0,
      "opacity": 0.82,
      "flip_h": false,
      "flip_v": false,
      "uniform_scale": true
    },
    "keyframes": [],
    "background_filling": null,
    "animations": null
  }
}
```

## 8. Transform 参数

对应结构：

- [Transform](./crates/jy_schema/src/transform.rs)

```json
{
  "x": 0.5,
  "y": 0.5,
  "scale_x": 1.0,
  "scale_y": 1.0,
  "rotation_deg": 0.0,
  "opacity": 1.0,
  "flip_h": false,
  "flip_v": false,
  "uniform_scale": true
}
```

### 8.1 坐标系说明

`x / y` 使用归一化坐标：

| 值 | 含义 |
|---|---|
| `x = 0.5` | 水平居中 |
| `y = 0.5` | 垂直居中 |
| `x = 0.0` | 最左侧 |
| `x = 1.0` | 最右侧 |
| `y = 0.0` | 最上侧 |
| `y = 1.0` | 最下侧 |

示例：

- 右上角水印常见取值：`x=0.86, y=0.11`
- 底部字幕常见取值：`x=0.5, y=0.09 ~ 0.12`

### 8.2 缩放与透明度

| 字段 | 说明 |
|---|---|
| `scale_x` | 水平缩放倍数 |
| `scale_y` | 垂直缩放倍数 |
| `opacity` | 透明度，`0.0 ~ 1.0` |

## 9. TextStyle 参数

对应结构：

- [TextStyle](./crates/jy_schema/src/text_style.rs)

```json
{
  "size": 7.2,
  "bold": false,
  "italic": false,
  "underline": false,
  "color": [1.0, 1.0, 1.0],
  "alpha": 1.0,
  "align": "Center",
  "vertical": false,
  "letter_spacing": 0,
  "line_spacing": 0,
  "auto_wrapping": true,
  "max_line_width": 0.82
}
```

### 9.1 字段说明

| 字段 | 说明 |
|---|---|
| `size` | 字号，当前是项目内部值，不是 CSS px |
| `color` | RGB 三元组，范围 `0.0 ~ 1.0` |
| `alpha` | 透明度 |
| `align` | `Left / Center / Right` |
| `vertical` | 是否竖排 |
| `letter_spacing` | 字间距 |
| `line_spacing` | 行间距 |
| `auto_wrapping` | 是否自动换行 |
| `max_line_width` | 最大行宽占画面宽度比例 |

## 10. 最小可用完整示例

下面这个例子适合后端直接照着拼：

```json
{
  "name": "backend_manifest_demo",
  "canvas": {
    "width": 1920,
    "height": 1080,
    "fps": 30
  },
  "maintrack_adsorb": true,
  "video_materials": [
    {
      "id": "video_main",
      "path": "D:/assets/main.mp4",
      "duration": 10000000,
      "width": 1920,
      "height": 1080,
      "kind": "Video",
      "crop": {
        "upper_left_x": 0.0,
        "upper_left_y": 0.0,
        "upper_right_x": 1.0,
        "upper_right_y": 0.0,
        "lower_left_x": 0.0,
        "lower_left_y": 1.0,
        "lower_right_x": 1.0,
        "lower_right_y": 1.0
      },
      "name": "main.mp4"
    },
    {
      "id": "img_watermark",
      "path": "D:/assets/logo.png",
      "duration": 10800000000,
      "width": 800,
      "height": 200,
      "kind": "Photo",
      "crop": {
        "upper_left_x": 0.0,
        "upper_left_y": 0.0,
        "upper_right_x": 1.0,
        "upper_right_y": 0.0,
        "lower_left_x": 0.0,
        "lower_left_y": 1.0,
        "lower_right_x": 1.0,
        "lower_right_y": 1.0
      },
      "name": "logo.png"
    }
  ],
  "audio_materials": [
    {
      "id": "audio_bgm",
      "path": "D:/assets/bgm.wav",
      "duration": 10000000,
      "name": "bgm.wav"
    }
  ],
  "tracks": [
    {
      "id": "track_video_main",
      "kind": "Video",
      "name": "main_video",
      "render_index": 0,
      "mute": false,
      "clips": [
        {
          "Video": {
            "id": "clip_video_main",
            "material_id": "video_main",
            "target_timerange": {
              "start": 0,
              "duration": 10000000
            },
            "source_timerange": {
              "start": 0,
              "duration": 10000000
            },
            "speed": {
              "id": "speed_video_main",
              "speed": 1.0
            },
            "volume": 1.0,
            "change_pitch": false,
            "transform": {
              "x": 0.5,
              "y": 0.5,
              "scale_x": 1.0,
              "scale_y": 1.0,
              "rotation_deg": 0.0,
              "opacity": 1.0,
              "flip_h": false,
              "flip_v": false,
              "uniform_scale": true
            },
            "keyframes": [],
            "fade": null,
            "effects": [],
            "filters": [],
            "mask": null,
            "transition": null,
            "background_filling": null,
            "animations": null,
            "mix_mode": null
          }
        }
      ]
    },
    {
      "id": "track_video_overlay",
      "kind": "Video",
      "name": "watermark",
      "render_index": 100,
      "mute": false,
      "clips": [
        {
          "Image": {
            "id": "clip_watermark",
            "material_id": "img_watermark",
            "target_timerange": {
              "start": 0,
              "duration": 10000000
            },
            "source_timerange": {
              "start": 0,
              "duration": 10000000
            },
            "speed": {
              "id": "speed_watermark",
              "speed": 1.0
            },
            "transform": {
              "x": 0.86,
              "y": 0.11,
              "scale_x": 0.22,
              "scale_y": 0.22,
              "rotation_deg": 0.0,
              "opacity": 0.82,
              "flip_h": false,
              "flip_v": false,
              "uniform_scale": true
            },
            "keyframes": [],
            "background_filling": null,
            "animations": null
          }
        }
      ]
    },
    {
      "id": "track_audio_bgm",
      "kind": "Audio",
      "name": "bgm",
      "render_index": 0,
      "mute": false,
      "clips": [
        {
          "Audio": {
            "id": "clip_bgm",
            "material_id": "audio_bgm",
            "target_timerange": {
              "start": 0,
              "duration": 10000000
            },
            "source_timerange": {
              "start": 0,
              "duration": 10000000
            },
            "speed": {
              "id": "speed_bgm",
              "speed": 1.0
            },
            "volume": 0.25,
            "change_pitch": false,
            "keyframes": [],
            "fade": null,
            "effects": []
          }
        }
      ]
    },
    {
      "id": "track_text_subtitle",
      "kind": "Text",
      "name": "subtitle",
      "render_index": 15000,
      "mute": false,
      "clips": [
        {
          "Text": {
            "id": "clip_subtitle_1",
            "material_id": "text_mat_1",
            "target_timerange": {
              "start": 0,
              "duration": 2500000
            },
            "text": "这是第一句字幕",
            "font": null,
            "style": {
              "size": 7.2,
              "bold": false,
              "italic": false,
              "underline": false,
              "color": [1.0, 1.0, 1.0],
              "alpha": 1.0,
              "align": "Center",
              "vertical": false,
              "letter_spacing": 0,
              "line_spacing": 0,
              "auto_wrapping": true,
              "max_line_width": 0.82
            },
            "transform": {
              "x": 0.5,
              "y": 0.1,
              "scale_x": 1.0,
              "scale_y": 1.0,
              "rotation_deg": 0.0,
              "opacity": 1.0,
              "flip_h": false,
              "flip_v": false,
              "uniform_scale": true
            },
            "keyframes": [],
            "border": {
              "alpha": 1.0,
              "color": [0.0, 0.0, 0.0],
              "width": 0.08
            },
            "background": null,
            "shadow": {
              "alpha": 0.35,
              "color": [0.0, 0.0, 0.0],
              "diffuse": 18.0,
              "distance": 5.0,
              "angle": -45.0
            },
            "animations": null,
            "bubble": null,
            "effect": null
          }
        }
      ]
    }
  ]
}
```

## 11. 后端接入建议

如果你准备让后端生成 manifest，建议按这个顺序处理：

1. 先生成稳定的素材 ID
2. 把所有素材统一落成 `video_materials / audio_materials`
3. 再按时间轴生成 `tracks[].clips[]`
4. 所有时间统一用微秒
5. 所有路径统一用本机绝对路径

### 11.1 关于素材路径

这一点非常关键：

**manifest 里的 `path` 必须是生成草稿那台机器上真实存在的本机路径。**

也就是说：

- 如果你在服务端生成草稿，路径会是服务端路径
- 用户把草稿拿到自己电脑上，大概率打不开素材

所以更合理的方案通常是：

1. 后端输出 manifest 或 timeline 数据
2. 本地端下载素材
3. 本地端生成最终草稿

### 11.2 关于 ID

所有这些字段建议由后端稳定生成：

- `project.id`
- `track.id`
- `clip.id`
- `speed.id`
- `material.id`

原因是：

- 便于后续模板替换
- 便于调试和排查
- 便于增量更新

### 11.3 关于最小可用策略

如果你只是第一版想跑通，建议先只支持：

- `Video`
- `Audio`
- `Text`
- `Image`

并统一固定：

- `keyframes: []`
- `effects: []`
- `filters: []`
- `fade: null`
- `animations: null`
- `mask: null`
- `transition: null`
- `background_filling: null`
- `mix_mode: null`

这样最稳，也最容易先让后端落地。

## 12. 当前限制

当前 `generate` 命令是“直接按 schema 反序列化”的模式，所以：

- 顶层只有少数字段带默认值
- clip 内部结构基本都要求完整

换句话说：

**它更像一个“底层 manifest 接口”，不是一个宽松 DSL。**

如果后面你希望后端更轻松一些，可以继续往前做一层更友好的输入协议，比如：

- 只填 `timeline_in / timeline_out`
- 自动补 `speed`
- 自动补 `source_timerange`
- 自动补默认 transform
- 自动补空数组和 `null`

然后再由 Rust 把那份“简化输入”转换成当前 manifest。
