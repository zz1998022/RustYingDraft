import "./styles.css";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type BundleType =
  | "draft_package"
  | "timeline_package"
  | "simple_timeline_package"
  | "pipeline_package";

type BundleInspection = {
  source: string;
  bundle_root: string;
  bundle_type: BundleType | string;
  timeline_file: string | null;
  source_draft_dir: string | null;
  project_id: string | null;
  project_name: string | null;
  asset_count: number;
  track_count: number;
  asset_kinds: string[];
};

type ImportBundleSummary = {
  source: string;
  bundle_root: string;
  bundle_type: BundleType | string;
  timeline_file: string | null;
  source_draft_dir: string | null;
  draft_dir: string;
  project_id: string;
  name: string;
  duration: number;
  track_count: number;
  asset_count: number;
  video_material_count: number;
  audio_material_count: number;
};

const sourcePathInput = document.querySelector<HTMLInputElement>("#source-path")!;
const draftBoxInput = document.querySelector<HTMLInputElement>("#draft-box-dir")!;
const draftNameInput = document.querySelector<HTMLInputElement>("#draft-name")!;
const summaryPanel = document.querySelector<HTMLDivElement>("#bundle-summary")!;
const statusLine = document.querySelector<HTMLParagraphElement>("#status-line")!;
const resultPanel = document.querySelector<HTMLPreElement>("#result-panel")!;
const importButton = document.querySelector<HTMLButtonElement>("#run-import")!;
const openDraftDirButton = document.querySelector<HTMLButtonElement>("#open-draft-dir")!;
const toast = document.querySelector<HTMLDivElement>("#toast")!;

let currentInspection: BundleInspection | null = null;
let latestDraftDir: string | null = null;
let lastAutoDraftName: string | null = null;
let draftNameWasEdited = false;
let toastTimer: number | null = null;

boot().catch((error) => {
  setStatus(`初始化失败：${stringifyError(error)}`);
});

document
  .querySelector<HTMLButtonElement>("#pick-source-dir")!
  .addEventListener("click", async () => {
    const picked = await open({
      multiple: false,
      directory: true,
    });
    if (typeof picked === "string") {
      sourcePathInput.value = picked;
      await inspectCurrentSource();
    }
  });

importButton.addEventListener("click", async () => {
  await runImport();
});

draftNameInput.addEventListener("input", () => {
  const current = draftNameInput.value.trim();
  draftNameWasEdited = Boolean(current && current !== lastAutoDraftName);
});

openDraftDirButton.addEventListener("click", async () => {
  if (!latestDraftDir) {
    return;
  }

  try {
    await invoke("open_path_in_file_manager", { path: latestDraftDir });
  } catch (error) {
    const message = toUserMessage(error);
    resultPanel.textContent = message;
    setStatus("打开目录失败。");
    showToast(message, "error");
  }
});

async function boot(): Promise<void> {
  const hasDraftBox = await fillDetectedDraftBox();
  const message = hasDraftBox ? "请选择下载好的草稿项目目录。" : "没有检测到剪映草稿箱，请先打开剪映后重试。";
  setStatus(message);
  if (!hasDraftBox) {
    showToast(message, "error");
  }
}

async function fillDetectedDraftBox(): Promise<boolean> {
  const detected = await invoke<string | null>("detect_draft_box_dir");
  if (detected) {
    draftBoxInput.value = detected;
    return true;
  }

  draftBoxInput.value = "";
  return false;
}

async function inspectCurrentSource(): Promise<void> {
  const source = sourcePathInput.value.trim();
  if (!source) {
    setStatus("请先选择项目目录。");
    return;
  }

  latestDraftDir = null;
  openDraftDirButton.classList.add("is-hidden");
  setStatus("正在读取项目目录...");

  try {
    const inspection = await invoke<BundleInspection>("inspect_bundle_source", { source });
    currentInspection = inspection;
    summaryPanel.classList.remove("empty");
    summaryPanel.innerHTML = [
      `<strong>${inspection.project_name ?? "已读取项目"}</strong>`,
      `类型：${getBundleTypeLabel(inspection.bundle_type)}`,
      `素材 ${inspection.asset_count} 个`,
      `素材类型：${summarizeAssetKinds(inspection.asset_kinds)}`,
      `轨道 ${inspection.track_count} 条`,
    ].join("<br>");

    const nextDraftName = normalizeDraftName(
      inspection.project_name ?? inspection.project_id ?? "imported_bundle",
    );
    const currentDraftName = draftNameInput.value.trim();
    const shouldUseAutoName =
      !currentDraftName || !draftNameWasEdited || currentDraftName === lastAutoDraftName;

    lastAutoDraftName = nextDraftName;
    if (shouldUseAutoName) {
      draftNameInput.value = nextDraftName;
      draftNameWasEdited = false;
      showToast(`已自动更新草稿名：${nextDraftName}`, "success");
    }

    setStatus("项目没问题，可以开始生成了。");
  } catch (error) {
    currentInspection = null;
    summaryPanel.classList.add("empty");
    summaryPanel.textContent = "这个项目暂时读不了，请换一个再试。";
    const message = toUserMessage(error);
    resultPanel.textContent = message;
    setStatus("检查失败。");
    showToast(message, "error");
  }
}

async function runImport(): Promise<void> {
  const source = sourcePathInput.value.trim();
  const draftBoxDir = draftBoxInput.value.trim();
  const draftName = draftNameInput.value.trim();

  if (!source || !draftName) {
    const message = "请先选择项目目录，并确认草稿名。";
    setStatus(message);
    showToast(message, "warning");
    return;
  }

  if (!draftBoxDir) {
    const message = "没有检测到剪映草稿箱，请先打开剪映后重试。";
    setStatus(message);
    showToast(message, "error");
    return;
  }

  importButton.disabled = true;
  latestDraftDir = null;
  openDraftDirButton.classList.add("is-hidden");
  resultPanel.textContent = "正在生成，请稍候...";
  setStatus("正在生成草稿...");

  try {
    const summary = await invoke<ImportBundleSummary>("import_bundle_to_draft_box", {
      source,
      draftBoxDir,
      draftName,
    });
    latestDraftDir = summary.draft_dir;
    openDraftDirButton.classList.remove("is-hidden");
    resultPanel.textContent = [
      "已生成成功。",
      "",
      `草稿名：${summary.name}`,
      `位置：${summary.draft_dir}`,
      "",
      "现在可以去剪映里查看了。",
    ].join("\n");
    setStatus("生成完成。");
    showToast("草稿生成成功，可以去剪映里查看了。", "success");
  } catch (error) {
    latestDraftDir = null;
    openDraftDirButton.classList.add("is-hidden");
    const message = toUserMessage(error);
    resultPanel.textContent = message;
    setStatus("生成失败。");
    showToast(message, "error");
  } finally {
    importButton.disabled = false;
  }
}

function normalizeDraftName(value: string): string {
  const trimmed = value.trim();
  const sanitized = trimmed.replace(/[\\/:*?"<>|]+/g, "_").replace(/\s+/g, "_");
  return sanitized || "imported_bundle";
}

function getBundleTypeLabel(bundleType: string): string {
  switch (bundleType) {
    case "draft_package":
      return "现有草稿包";
    case "timeline_package":
      return "时间轴项目包";
    case "simple_timeline_package":
      return "简化时间轴包";
    case "pipeline_package":
      return "后端流水线包";
    default:
      return bundleType || "未知项目包";
  }
}

function summarizeAssetKinds(assetKinds: string[]): string {
  if (assetKinds.length === 0) {
    return "无";
  }

  const counts = assetKinds.reduce<Record<string, number>>((accumulator, kind) => {
    accumulator[kind] = (accumulator[kind] ?? 0) + 1;
    return accumulator;
  }, {});

  const labels: Record<string, string> = {
    video: "视频",
    audio: "音频",
    image: "图片",
    sticker: "贴纸",
  };

  return Object.entries(counts)
    .map(([kind, count]) => `${labels[kind] ?? kind} ${count}`)
    .join("，");
}

function setStatus(message: string): void {
  statusLine.textContent = message;
}

function stringifyError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return JSON.stringify(error, null, 2);
}

function toUserMessage(error: unknown): string {
  const raw = stringifyError(error);
  if (/output directory is not empty/i.test(raw)) {
    return "这个草稿项目已经存在，请换一个草稿名，或者先删除剪映草稿箱里的同名草稿。";
  }
  if (/pipeline\.audio_file is no longer supported/i.test(raw)) {
    return "这个项目包还是旧版单音频格式，请把 bundle.json 改成 pipeline.narration_files 分段解说音频列表。";
  }
  if (/pipeline\.narration_files length/i.test(raw)) {
    return "解说音频数量和 SRT 字幕条数不一致，请确认 narration_files 的顺序和数量都对应 subtitle.srt。";
  }
  if (/pipeline\.narration_files must contain at least one/i.test(raw)) {
    return "这个项目包缺少分段解说音频，请在 bundle.json 里填写 pipeline.narration_files。";
  }
  if (/concat\.txt is empty/i.test(raw)) {
    return "concat.txt 里没有视频条目，请至少写一行 file 'video_001.mp4'。";
  }
  if (/path must use forward slash|path must not contain backslash/i.test(raw)) {
    return "项目包里的素材路径必须使用正斜杠 /，不要使用 Windows 反斜杠。";
  }
  if (/path must be relative|absolute path|url/i.test(raw)) {
    return "项目包里的素材路径必须是相对 assets 目录的本地路径，不能使用绝对路径或 URL。";
  }
  if (/subtitle .* exceeds video duration|subtitle.*out of/i.test(raw)) {
    return "SRT 字幕时间超过了拼接后视频总时长，请检查 subtitle.srt 的结束时间。";
  }
  if (/ffprobe/i.test(raw)) {
    return `读取媒体信息失败，导入插件自带的媒体探测组件缺失或无法运行。请重新安装最新版导入插件后重试，仍不行请联系支持。原始错误：${raw}`;
  }
  if (/failed to inspect directory/i.test(raw) || /No such file or directory/i.test(raw)) {
    return "这个项目目录读不了，请确认选择的是下载好的草稿目录。";
  }
  return raw || "操作失败，请换一个项目再试。";
}

function showToast(message: string, tone: "success" | "warning" | "error" = "success"): void {
  if (toastTimer !== null) {
    window.clearTimeout(toastTimer);
  }

  toast.textContent = message;
  toast.className = `toast toast-${tone} is-visible`;
  toastTimer = window.setTimeout(() => {
    toast.classList.remove("is-visible");
    toastTimer = null;
  }, 4200);
}
