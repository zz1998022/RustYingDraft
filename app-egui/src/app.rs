use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use eframe::egui;
use jy_bundle::{
    inspect_bundle_source, ImportBundleOptions, ImportBundleProgress, ImportBundleSummary,
};

use crate::job::{spawn_import_job, ImportJobEvent};
use crate::platform::{
    detect_bundle_candidates, detect_known_draft_box_dirs, open_path_in_file_manager,
};
use crate::util::{path_row, resolve_picked_source, sanitize_draft_name};

pub(crate) struct YingDraftApp {
    source: String,
    draft_box_dir: String,
    draft_name: String,
    status: String,
    stage: String,
    current_path: String,
    progress_current: usize,
    progress_total: usize,
    started_at: Option<Instant>,
    finished_elapsed: Option<Duration>,
    // 后台导入任务的事件通道与取消标志；非导入态为 None。
    receiver: Option<mpsc::Receiver<ImportJobEvent>>,
    cancel_flag: Option<Arc<AtomicBool>>,
    importing: bool,
    cancelling: bool,
    error: String,
    summary: Option<ImportBundleSummary>,
}

impl YingDraftApp {
    pub(crate) fn new() -> Self {
        // 启动时预填充：草稿箱用常见默认目录，草稿包就近探测，减少用户手动输入。
        let draft_box_dir = detect_known_draft_box_dirs()
            .into_iter()
            .find(|path| path.exists() && path.is_dir())
            .and_then(|path| path.to_str().map(str::to_string))
            .unwrap_or_default();
        let source = detect_bundle_candidates()
            .into_iter()
            .find(|path| path.join("bundle.json").exists())
            .and_then(|path| path.to_str().map(str::to_string))
            .unwrap_or_default();

        Self {
            source,
            draft_box_dir,
            draft_name: "imported_bundle".to_string(),
            status: "请选择草稿包和剪映草稿箱目录".to_string(),
            stage: String::new(),
            current_path: String::new(),
            progress_current: 0,
            progress_total: 0,
            started_at: None,
            finished_elapsed: None,
            receiver: None,
            cancel_flag: None,
            importing: false,
            cancelling: false,
            error: String::new(),
            summary: None,
        }
    }

    fn inspect_source(&mut self) {
        self.error.clear();
        let source = Utf8PathBuf::from(self.source.trim());
        match inspect_bundle_source(&source) {
            Ok(inspection) => {
                if let Some(name) = inspection.project_name {
                    self.draft_name = name;
                }
                self.status = format!(
                    "包类型：{}，轨道：{}，素材：{}",
                    inspection.bundle_type, inspection.track_count, inspection.asset_count
                );
            }
            Err(error) => {
                self.error = format!("{error:#}");
                self.status = "读取包信息失败".to_string();
            }
        }
    }

    fn start_import(&mut self, ctx: &egui::Context) {
        self.error.clear();
        self.summary = None;
        let source = self.source.trim().to_string();
        let draft_box_dir = self.draft_box_dir.trim().to_string();
        let draft_name = sanitize_draft_name(&self.draft_name);
        if source.is_empty() || draft_box_dir.is_empty() || draft_name.is_empty() {
            self.error = "source、draft_box_dir、draft_name must not be empty".to_string();
            return;
        }

        let output = Utf8PathBuf::from(draft_box_dir).join(&draft_name);
        let options = ImportBundleOptions {
            source: Utf8PathBuf::from(source),
            output,
            name_override: Some(draft_name),
        };

        let (receiver, cancel_flag) = spawn_import_job(options, ctx.clone());
        self.receiver = Some(receiver);
        self.cancel_flag = Some(cancel_flag);
        self.importing = true;
        self.cancelling = false;
        self.started_at = Some(Instant::now());
        self.finished_elapsed = None;
        self.progress_current = 0;
        self.progress_total = 0;
        self.stage.clear();
        self.current_path.clear();
        self.status = "正在导入".to_string();
        ctx.request_repaint();
    }

    fn cancel_import(&mut self, ctx: &egui::Context) {
        if let Some(flag) = &self.cancel_flag {
            flag.store(true, Ordering::Relaxed);
            self.cancelling = true;
            self.status = "正在取消，等待当前步骤结束".to_string();
            ctx.request_repaint();
        }
    }

    fn poll_job_events(&mut self) {
        // 每帧把后台线程发来的事件取空；收到 Finished 后丢弃 receiver 结束本次任务。
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        while let Ok(event) = receiver.try_recv() {
            match event {
                ImportJobEvent::Progress(progress) => self.apply_progress(progress),
                ImportJobEvent::Finished(result) => {
                    self.finish_import(result);
                    keep_receiver = false;
                }
            }
        }
        if keep_receiver {
            self.receiver = Some(receiver);
        }
    }

    fn finish_import(&mut self, result: Result<ImportBundleSummary, String>) {
        self.finished_elapsed = self.started_at.map(|started_at| started_at.elapsed());
        self.started_at = None;
        self.importing = false;
        self.cancelling = false;
        self.cancel_flag = None;
        match result {
            Ok(summary) => {
                self.status = "导入完成".to_string();
                self.current_path.clear();
                self.progress_current = self.progress_total;
                self.summary = Some(summary);
            }
            Err(error) => {
                // jy_bundle 取消会返回带 "import cancelled" 的错误，与真正失败区分开。
                self.status = if error.contains("import cancelled") {
                    "导入已取消".to_string()
                } else {
                    "导入失败".to_string()
                };
                self.error = error;
            }
        }
    }

    fn elapsed_duration(&self) -> Option<Duration> {
        self.finished_elapsed
            .or_else(|| self.started_at.map(|started_at| started_at.elapsed()))
    }

    fn apply_progress(&mut self, progress: ImportBundleProgress) {
        // 进度事件的结构化字段放在 data map 里，按 current/total/path 取出驱动进度条与当前文件。
        self.stage = progress.stage;
        self.status = progress.message;
        if let Some(current) = progress
            .data
            .get("current")
            .and_then(|value| value.as_u64())
        {
            self.progress_current = current as usize;
        }
        if let Some(total) = progress.data.get("total").and_then(|value| value.as_u64()) {
            self.progress_total = total as usize;
        }
        if let Some(path) = progress.data.get("path").and_then(|value| value.as_str()) {
            self.current_path = path.to_string();
        }
    }
}

impl eframe::App for YingDraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job_events();
        if self.importing {
            // 导入中持续定时重绘，保证进度条和耗时实时更新。
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("YingDraft 导入器");
            ui.add_space(8.0);

            ui.add_enabled_ui(!self.importing, |ui| {
                // 草稿包来源支持目录 / .zip / bundle.json 三种，和 jy_bundle 接受的形态一致。
                ui.horizontal(|ui| {
                    ui.label("草稿包");
                    ui.text_edit_singleline(&mut self.source);
                    if ui.button("选目录").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.source = path.display().to_string();
                        }
                    }
                    if ui.button("选文件").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("草稿包 (.zip / bundle.json)", &["zip", "json"])
                            .pick_file()
                        {
                            self.source = resolve_picked_source(&path);
                        }
                    }
                });
                path_row(
                    ui,
                    "剪映草稿箱目录",
                    &mut self.draft_box_dir,
                    "选择",
                    || {
                        rfd::FileDialog::new()
                            .pick_folder()
                            .map(|path| path.display().to_string())
                    },
                );
                ui.horizontal(|ui| {
                    ui.label("草稿名");
                    ui.text_edit_singleline(&mut self.draft_name);
                });
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.importing, egui::Button::new("读取包信息"))
                    .clicked()
                {
                    self.inspect_source();
                }
                if ui
                    .add_enabled(!self.importing, egui::Button::new("开始导入"))
                    .clicked()
                {
                    self.start_import(ctx);
                }
                if ui
                    .add_enabled(
                        self.importing && !self.cancelling,
                        egui::Button::new("取消导入"),
                    )
                    .clicked()
                {
                    self.cancel_import(ctx);
                }
            });

            ui.separator();
            ui.label(format!("状态：{}", self.status));
            if !self.stage.is_empty() {
                ui.label(format!("阶段：{}", self.stage));
            }
            if let Some(elapsed) = self.elapsed_duration() {
                ui.label(format!("耗时：{:.1}s", elapsed.as_secs_f32()));
            }
            if self.progress_total > 0 {
                let ratio = self.progress_current as f32 / self.progress_total as f32;
                ui.add(
                    egui::ProgressBar::new(ratio.clamp(0.0, 1.0))
                        .text(format!("{}/{}", self.progress_current, self.progress_total)),
                );
            }
            if !self.current_path.is_empty() {
                ui.label(format!("当前文件：{}", self.current_path));
            }

            if let Some(summary) = &self.summary {
                ui.separator();
                ui.label(format!("输出目录：{}", summary.draft_dir));
                ui.label(format!(
                    "轨道：{}，素材：{}，视频素材：{}，音频素材：{}",
                    summary.track_count,
                    summary.asset_count,
                    summary.video_material_count,
                    summary.audio_material_count
                ));
                if ui.button("打开输出目录").clicked() {
                    let _ = open_path_in_file_manager(&summary.draft_dir);
                }
            }

            if !self.error.is_empty() {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("错误详情");
                    if ui.button("复制").clicked() {
                        ui.ctx().copy_text(self.error.clone());
                    }
                });
                ui.add(
                    egui::TextEdit::multiline(&mut self.error)
                        .desired_rows(7)
                        .code_editor(),
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_import_freezes_elapsed_time() {
        let mut app = YingDraftApp::new();
        app.importing = true;
        app.started_at = Some(Instant::now() - Duration::from_secs(2));

        app.finish_import(Err(
            "pipeline.tracks[0].clips[0].end is not supported for video tracks".to_string(),
        ));

        let elapsed_after_finish = app.elapsed_duration().expect("elapsed is kept");
        std::thread::sleep(Duration::from_millis(20));
        let elapsed_later = app.elapsed_duration().expect("elapsed is kept");

        assert!(!app.importing);
        assert_eq!(app.status, "导入失败");
        assert_eq!(elapsed_after_finish, elapsed_later);
    }
}
