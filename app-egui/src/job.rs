use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use eframe::egui;
use jy_bundle::{
    import_bundle_with_progress_and_cancel, ImportBundleOptions, ImportBundleProgress,
    ImportBundleSummary,
};

pub(crate) enum ImportJobEvent {
    Progress(ImportBundleProgress),
    Finished(Result<ImportBundleSummary, String>),
}

/// 在后台线程执行导入，避免阻塞 UI 线程。
///
/// 通过 channel 把进度/结果事件回传给 UI，用 `AtomicBool` 做协作式取消
/// （由 jy_bundle 在各步骤之间轮询）。返回 receiver 与取消标志交调用方持有。
pub(crate) fn spawn_import_job(
    options: ImportBundleOptions,
    ctx: egui::Context,
) -> (mpsc::Receiver<ImportJobEvent>, Arc<AtomicBool>) {
    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_for_job = Arc::clone(&cancel_flag);

    std::thread::spawn(move || {
        let progress_sender = sender.clone();
        let progress_ctx = ctx.clone();
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
        send_job_event(&sender, &ctx, ImportJobEvent::Finished(result));
    });

    (receiver, cancel_flag)
}

fn send_job_event(
    sender: &mpsc::Sender<ImportJobEvent>,
    ctx: &egui::Context,
    event: ImportJobEvent,
) {
    // 发完事件主动请求重绘，唤醒 UI 线程及时取走事件并刷新界面。
    let _ = sender.send(event);
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
