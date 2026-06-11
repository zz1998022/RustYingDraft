use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use eframe::egui;
use jy_bundle::{
    import_bundle_with_progress_and_cancel, inspect_bundle_source, ImportBundleOptions,
    ImportBundleProgress, ImportBundleSummary,
};

const BUILTIN_CJK_FONT_NAME: &str = "noto_sans_cjk_sc_regular";
const BUILTIN_CJK_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");

fn main() -> eframe::Result {
    configure_bundled_ffprobe();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 520.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };

    eframe::run_native(
        "YingDraft 导入器",
        native_options,
        Box::new(|cc| {
            install_builtin_fonts(&cc.egui_ctx);
            Ok(Box::new(YingDraftApp::new()))
        }),
    )
}

fn install_builtin_fonts(ctx: &egui::Context) {
    ctx.set_fonts(builtin_font_definitions());
}

fn builtin_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        BUILTIN_CJK_FONT_NAME.to_string(),
        egui::FontData::from_static(BUILTIN_CJK_FONT_BYTES).into(),
    );

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, BUILTIN_CJK_FONT_NAME.to_string());
    }

    fonts
}

struct YingDraftApp {
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
    receiver: Option<mpsc::Receiver<ImportJobEvent>>,
    cancel_flag: Option<Arc<AtomicBool>>,
    importing: bool,
    cancelling: bool,
    error: String,
    summary: Option<ImportBundleSummary>,
}

impl YingDraftApp {
    fn new() -> Self {
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

        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_for_job = Arc::clone(&cancel_flag);
        let repaint_ctx = ctx.clone();
        std::thread::spawn(move || {
            let progress_sender = sender.clone();
            let progress_ctx = repaint_ctx.clone();
            let result = import_bundle_with_progress_and_cancel(
                &options,
                move |event| {
                    send_job_event(
                        &progress_sender,
                        &progress_ctx,
                        ImportJobEvent::Progress(event),
                    );
                },
                || cancel_for_job.load(Ordering::Relaxed),
            )
            .map_err(|error| format!("{error:#}"));
            send_job_event(&sender, &repaint_ctx, ImportJobEvent::Finished(result));
        });

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
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("YingDraft 导入器");
            ui.add_space(8.0);

            ui.add_enabled_ui(!self.importing, |ui| {
                path_row(ui, "草稿包目录", &mut self.source, "选择", || {
                    rfd::FileDialog::new()
                        .pick_folder()
                        .map(|path| path.display().to_string())
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

enum ImportJobEvent {
    Progress(ImportBundleProgress),
    Finished(Result<ImportBundleSummary, String>),
}

fn send_job_event(
    sender: &mpsc::Sender<ImportJobEvent>,
    ctx: &egui::Context,
    event: ImportJobEvent,
) {
    let _ = sender.send(event);
    ctx.request_repaint();
}

fn path_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    button: &str,
    pick: impl FnOnce() -> Option<String>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
        if ui.button(button).clicked() {
            if let Some(path) = pick() {
                *value = path;
            }
        }
    });
}

fn sanitize_draft_name(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");

    if sanitized.is_empty() {
        "imported_bundle".to_string()
    } else {
        sanitized
    }
}

fn detect_known_draft_box_dirs() -> Vec<std::path::PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    if cfg!(target_os = "macos") {
        candidates.push(
            home.join("Movies")
                .join("JianyingPro")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
        candidates.push(
            home.join("Movies")
                .join("CapCut")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
    }
    if cfg!(target_os = "windows") {
        candidates.push(
            home.join("AppData")
                .join("Local")
                .join("JianyingPro")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
        candidates.push(
            home.join("AppData")
                .join("Local")
                .join("CapCut")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
    }
    candidates
}

fn detect_bundle_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir);
    }
    if let Ok(current_exe) = std::env::current_exe() {
        for ancestor in current_exe.ancestors().skip(1).take(6) {
            candidates.push(ancestor.to_path_buf());
        }
    }
    dedup_paths(candidates)
}

fn dedup_paths(paths: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    for path in paths {
        if !result.iter().any(|existing| existing == &path) {
            result.push(path);
        }
    }
    result
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from))
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut full = std::path::PathBuf::from(drive);
            full.push(path);
            Some(full)
        })
}

fn configure_bundled_ffprobe() {
    if std::env::var_os("JY_FFPROBE_PATH").is_some() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let binary = if cfg!(target_os = "windows") {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    let candidate = dir.join(binary);
    if candidate.exists() {
        std::env::set_var("JY_FFPROBE_PATH", candidate);
    }
}

fn open_path_in_file_manager(path: &str) -> Result<(), String> {
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).status()
    } else {
        Command::new("xdg-open").arg(path).status()
    }
    .map_err(|error| error.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to open path: {path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_font_is_registered_for_text_and_error_details() {
        let fonts = builtin_font_definitions();
        let proportional = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .expect("proportional font family exists");
        let monospace = fonts
            .families
            .get(&egui::FontFamily::Monospace)
            .expect("monospace font family exists");

        assert!(fonts.font_data.contains_key(BUILTIN_CJK_FONT_NAME));
        assert_eq!(
            proportional.first().map(String::as_str),
            Some(BUILTIN_CJK_FONT_NAME)
        );
        assert_eq!(
            monospace.first().map(String::as_str),
            Some(BUILTIN_CJK_FONT_NAME)
        );
    }

    #[test]
    fn send_job_event_delivers_event_to_ui_receiver() {
        let (sender, receiver) = mpsc::channel();
        let ctx = egui::Context::default();

        send_job_event(
            &sender,
            &ctx,
            ImportJobEvent::Finished(Err("expected failure".to_string())),
        );

        match receiver.try_recv().expect("event delivered") {
            ImportJobEvent::Finished(Err(error)) => assert_eq!(error, "expected failure"),
            _ => panic!("unexpected job event"),
        }
    }

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
