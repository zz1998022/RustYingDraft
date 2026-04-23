import "./styles.css";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type BundleInspection = {
  source: string;
  bundle_root: string;
  bundle_type: string;
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
  bundle_type: string;
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
const inspectButton = document.querySelector<HTMLButtonElement>("#inspect-source")!;
const openDraftDirButton = document.querySelector<HTMLButtonElement>("#open-draft-dir")!;

let currentInspection: BundleInspection | null = null;
let latestDraftDir: string | null = null;

boot().catch((error) => {
  setStatus(`初始化失败：${stringifyError(error)}`);
});

document
  .querySelector<HTMLButtonElement>("#detect-source-nearby")!
  .addEventListener("click", async () => {
    await fillDetectedSource();
  });

document
  .querySelector<HTMLButtonElement>("#pick-source-file")!
  .addEventListener("click", async () => {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [
        { name: "Project Bundle", extensions: ["zip", "json"] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (typeof picked === "string") {
      sourcePathInput.value = picked;
      await inspectCurrentSource();
    }
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

document
  .querySelector<HTMLButtonElement>("#pick-draft-box")!
  .addEventListener("click", async () => {
    const picked = await open({
      multiple: false,
      directory: true,
    });
    if (typeof picked === "string") {
      draftBoxInput.value = picked;
    }
  });

document
  .querySelector<HTMLButtonElement>("#detect-draft-box")!
  .addEventListener("click", async () => {
    await fillDetectedDraftBox();
  });

inspectButton.addEventListener("click", async () => {
  await inspectCurrentSource();
});

importButton.addEventListener("click", async () => {
  await runImport();
});

openDraftDirButton.addEventListener("click", async () => {
  if (!latestDraftDir) {
    return;
  }

  try {
    await invoke("open_path_in_file_manager", { path: latestDraftDir });
  } catch (error) {
    resultPanel.textContent = stringifyError(error);
    setStatus("打开目录失败。");
  }
});

async function boot(): Promise<void> {
  await fillDetectedDraftBox();
  const detectedSource = await fillDetectedSource();
  if (!detectedSource) {
    setStatus("等待选择项目包。");
  }
}

async function fillDetectedDraftBox(): Promise<void> {
  const detected = await invoke<string | null>("detect_draft_box_dir");
  if (detected) {
    draftBoxInput.value = detected;
    setStatus("已自动检测到一个常见的剪映草稿箱目录。");
  } else {
    setStatus("暂时没有检测到默认草稿箱目录，可以手动选择。");
  }
}

async function fillDetectedSource(): Promise<boolean> {
  const detected = await invoke<string | null>("detect_bundle_source_near_app");
  if (!detected) {
    return false;
  }

  sourcePathInput.value = detected;
  await inspectCurrentSource();
  return true;
}

async function inspectCurrentSource(): Promise<void> {
  const source = sourcePathInput.value.trim();
  if (!source) {
    setStatus("请先选择项目包、项目目录，或者 bundle.json。");
    return;
  }

  inspectButton.disabled = true;
  latestDraftDir = null;
  openDraftDirButton.classList.add("is-hidden");
  setStatus("正在读取项目包信息...");

  try {
    const inspection = await invoke<BundleInspection>("inspect_bundle_source", { source });
    currentInspection = inspection;
    summaryPanel.classList.remove("empty");
    summaryPanel.innerHTML = [
      `<strong>${inspection.project_name ?? "已读取项目"}</strong>`,
      `类型：${inspection.bundle_type === "draft_package" ? "现有草稿包" : "时间轴项目包"}`,
      `素材 ${inspection.asset_count} 个`,
      `轨道 ${inspection.track_count} 条`,
    ].join("<br>");

    if (!draftNameInput.value.trim()) {
      draftNameInput.value = normalizeDraftName(
        inspection.project_name ?? inspection.project_id ?? "imported_bundle",
      );
    }

    setStatus("项目没问题，可以开始生成了。");
  } catch (error) {
    currentInspection = null;
    summaryPanel.classList.add("empty");
    summaryPanel.textContent = "这个项目暂时读不了，请换一个再试。";
    resultPanel.textContent = stringifyError(error);
    setStatus("检查失败。");
  } finally {
    inspectButton.disabled = false;
  }
}

async function runImport(): Promise<void> {
  const source = sourcePathInput.value.trim();
  const draftBoxDir = draftBoxInput.value.trim();
  const draftName = draftNameInput.value.trim();

  if (!source || !draftBoxDir || !draftName) {
    setStatus("请把项目来源、草稿箱目录和草稿名都填完整。");
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
  } catch (error) {
    latestDraftDir = null;
    openDraftDirButton.classList.add("is-hidden");
    resultPanel.textContent = stringifyError(error);
    setStatus("生成失败。");
  } finally {
    importButton.disabled = false;
  }
}

function normalizeDraftName(value: string): string {
  const trimmed = value.trim();
  const sanitized = trimmed.replace(/[\\/:*?"<>|]+/g, "_").replace(/\s+/g, "_");
  return sanitized || "imported_bundle";
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
