# Bundle 规范

本文定义本地导入工具读取的 `bundle.json`。

`bundle.json` 只描述打包结构和素材映射，不保存用户电脑上的绝对路径。

## 1. 包类型

`bundle_type` 当前支持两个值：

| 值 | 用途 |
| --- | --- |
| `draft_package` | 包内已经包含剪映草稿，本地导入时只重写素材路径 |
| `timeline_package` | 包内提供时间轴描述，本地导入时重新生成剪映草稿 |

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
| `bundle_type` | `string` | 是 | `draft_package` 或 `timeline_package` |
| `project_id` | `string` | 否 | 业务侧项目 ID |
| `project_name` | `string` | 否 | 草稿默认名称 |
| `assets_dir` | `string` | 否 | 素材目录，通常为 `assets` |
| `draft_dir` | `string` | `draft_package` 必填 | 草稿目录，通常为 `draft` |
| `timeline_file` | `string` | `timeline_package` 必填 | 时间轴描述文件 |

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

## 7. 后端出包流程

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

## 8. 校验清单

出包前建议检查：

- `bundle_version` 为 `1`
- `bundle_type` 为 `draft_package`
- `draft/draft_content.json` 存在
- `draft/draft_info.json` 存在
- `draft/draft_meta_info.json` 存在
- `assets[]` 中的每个 `relative_path` 都能找到文件
- `assets[]` 中的每个 `match_value` 都能在草稿素材列表中匹配到
- 素材名不要重复；如果重复，导入时会报歧义错误
