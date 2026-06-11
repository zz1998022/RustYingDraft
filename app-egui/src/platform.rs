use std::process::Command;

/// 启动时把随包 ffprobe 的绝对路径注入 `JY_FFPROBE_PATH`。
///
/// macOS GUI 进程不继承用户 shell 的 PATH，客户机器也未必装过 ffmpeg，
/// 因此优先指向发布包里与可执行文件同目录的 ffprobe；
/// 若用户已显式设置该变量或找不到随包二进制，则保持原状回退到 PATH。
pub(crate) fn configure_bundled_ffprobe() {
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

pub(crate) fn open_path_in_file_manager(path: &str) -> Result<(), String> {
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

/// 常见剪映 / CapCut 默认草稿箱目录，启动时用于预填充，省去用户手动找路径。
pub(crate) fn detect_known_draft_box_dirs() -> Vec<std::path::PathBuf> {
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

/// 在当前工作目录及可执行文件的上层目录里探测草稿包，
/// 适配「把导入器和草稿包放一起直接双击运行」的场景。
pub(crate) fn detect_bundle_candidates() -> Vec<std::path::PathBuf> {
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
    // Windows 上通常没有 HOME，依次回退到 USERPROFILE、再到 HOMEDRIVE + HOMEPATH 组合。
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
