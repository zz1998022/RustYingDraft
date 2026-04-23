use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8Path;
use serde::Serialize;
use tempfile::tempdir;
use uuid::Uuid;

use crate::output;

const DEFAULT_INSTALL_DIR_UNIX: &str = ".local/opt/YingDraft";
const RELEASE_ASSET_PREFIX: &str = "yingdraft";

#[derive(Debug, Serialize)]
struct UpdateSummary {
    repo: String,
    version: String,
    asset: String,
    install_dir: String,
    deferred: bool,
}

pub fn run(version: Option<&str>, repo: &str, install_dir: Option<&Utf8Path>) -> Result<()> {
    let version = version.unwrap_or("latest");
    let asset = detect_release_asset()?;
    let install_dir = resolve_install_dir(install_dir)?;
    let url = release_download_url(repo, version, &asset);

    output::emit_progress(
        "update",
        "resolve",
        &format!("准备更新 YingDraft CLI: {version}"),
        serde_json::json!({
            "repo": repo,
            "version": version,
            "asset": asset,
            "install_dir": install_dir.display().to_string(),
            "url": url,
        }),
    );

    if cfg!(windows) {
        update_windows(repo, version, &asset, &url, &install_dir)
    } else {
        update_unix(repo, version, &asset, &url, &install_dir)
    }
}

fn update_unix(
    repo: &str,
    version: &str,
    asset: &str,
    url: &str,
    install_dir: &Path,
) -> Result<()> {
    let temp_dir = tempdir().context("创建更新临时目录失败")?;
    let archive_path = temp_dir.path().join(asset);

    download_release_asset(url, &archive_path)?;
    extract_archive(&archive_path, temp_dir.path())?;
    let bundle_root = find_bundle_root(temp_dir.path())?;
    let installer = bundle_root.join("scripts").join("install-jy.sh");

    output::emit_progress(
        "update",
        "install",
        &format!("正在安装到 {}", install_dir.display()),
        serde_json::json!({ "installer": installer.display().to_string() }),
    );

    let status = Command::new("bash")
        .arg(&installer)
        .arg(install_dir)
        .status()
        .with_context(|| format!("执行安装脚本失败: {}", installer.display()))?;
    if !status.success() {
        bail!("安装脚本执行失败，退出码: {status}");
    }

    emit_success(repo, version, asset, install_dir, false);
    Ok(())
}

fn update_windows(
    repo: &str,
    version: &str,
    asset: &str,
    url: &str,
    install_dir: &Path,
) -> Result<()> {
    let temp_root = std::env::temp_dir().join(format!("yingdraft-update-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_root).context("创建 Windows 更新临时目录失败")?;
    let archive_path = temp_root.join(asset);

    download_release_asset(url, &archive_path)?;
    extract_archive(&archive_path, &temp_root)?;
    let bundle_root = find_bundle_root(&temp_root)?;
    let installer = bundle_root.join("scripts").join("install-jy.ps1");
    if !installer.is_file() {
        bail!("发布包结构不正确，未找到 scripts/install-jy.ps1");
    }

    let handoff = temp_root.join("run-update.ps1");
    let current_pid = std::process::id();
    write_windows_handoff_script(&handoff, current_pid, &installer, install_dir, &temp_root)?;

    output::emit_progress(
        "update",
        "handoff",
        "Windows 正在启动后台更新脚本，当前 CLI 退出后会继续覆盖安装",
        serde_json::json!({ "script": handoff.display().to_string() }),
    );

    let powershell = powershell_command()?;
    Command::new(powershell)
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&handoff)
        .spawn()
        .context("启动 Windows 后台更新脚本失败")?;

    emit_success(repo, version, asset, install_dir, true);
    Ok(())
}

fn download_release_asset(url: &str, output_path: &Path) -> Result<()> {
    output::emit_progress(
        "update",
        "download",
        "正在下载发布包...",
        serde_json::json!({ "url": url, "output": output_path.display().to_string() }),
    );

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .context("创建下载客户端失败")?;
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("下载发布包失败: {url}"))?
        .error_for_status()
        .with_context(|| format!("发布包下载地址不可用: {url}"))?;

    let mut file = File::create(output_path)
        .with_context(|| format!("创建下载文件失败: {}", output_path.display()))?;
    response.copy_to(&mut file).context("写入发布包文件失败")?;
    Ok(())
}

fn extract_archive(archive_path: &Path, output_dir: &Path) -> Result<()> {
    output::emit_progress(
        "update",
        "extract",
        "正在解压发布包...",
        serde_json::json!({
            "archive": archive_path.display().to_string(),
            "output": output_dir.display().to_string(),
        }),
    );

    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(output_dir)
        .status()
        .context("执行 tar 解压失败，请确认系统可以使用 tar 命令")?;
    if !status.success() {
        bail!("发布包解压失败，退出码: {status}");
    }
    Ok(())
}

fn find_bundle_root(search_root: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(search_root).context("读取发布包解压目录失败")? {
        let entry = entry?;
        let path = entry.path();
        if path.join("scripts").join("install-jy.sh").is_file()
            || path.join("scripts").join("install-jy.ps1").is_file()
        {
            return Ok(path);
        }
    }

    bail!("发布包结构不正确，未找到安装脚本");
}

fn resolve_install_dir(install_dir: Option<&Utf8Path>) -> Result<PathBuf> {
    if let Some(install_dir) = install_dir {
        return Ok(install_dir.as_std_path().to_path_buf());
    }

    if let Ok(value) = std::env::var("YINGDRAFT_INSTALL_DIR") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }

    if let Some(dir) = infer_install_dir_from_current_exe()? {
        return Ok(dir);
    }

    default_install_dir()
}

fn infer_install_dir_from_current_exe() -> Result<Option<PathBuf>> {
    let current_exe = std::env::current_exe().context("读取当前 CLI 路径失败")?;
    let Some(bin_dir) = current_exe.parent() else {
        return Ok(None);
    };
    if bin_dir.file_name().and_then(|name| name.to_str()) != Some("bin") {
        return Ok(None);
    }
    Ok(bin_dir.parent().map(Path::to_path_buf))
}

fn default_install_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .context("无法读取 LOCALAPPDATA，请用 --install-dir 指定安装目录")?;
        Ok(PathBuf::from(local_app_data).join("YingDraft"))
    } else {
        let home =
            std::env::var("HOME").context("无法读取 HOME，请用 --install-dir 指定安装目录")?;
        Ok(PathBuf::from(home).join(DEFAULT_INSTALL_DIR_UNIX))
    }
}

fn detect_release_asset() -> Result<String> {
    asset_name_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn asset_name_for(os: &str, arch: &str) -> Result<String> {
    let platform = match os {
        "macos" => match arch {
            "aarch64" | "arm64" => "macos-arm64",
            "x86_64" | "amd64" => "macos-x64",
            _ => bail!("暂不支持的 macOS 架构: {arch}"),
        },
        "linux" => match arch {
            "x86_64" | "amd64" => "linux-x64",
            _ => bail!("暂不支持的 Linux 架构: {arch}"),
        },
        "windows" => match arch {
            "x86_64" | "amd64" => "windows-x64",
            _ => bail!("暂不支持的 Windows 架构: {arch}"),
        },
        _ => bail!("当前 update 暂不支持该系统: {os}"),
    };

    Ok(format!("{RELEASE_ASSET_PREFIX}-{platform}.tar.gz"))
}

fn release_download_url(repo: &str, version: &str, asset: &str) -> String {
    if version == "latest" {
        format!("https://github.com/{repo}/releases/latest/download/{asset}")
    } else {
        format!("https://github.com/{repo}/releases/download/{version}/{asset}")
    }
}

fn emit_success(repo: &str, version: &str, asset: &str, install_dir: &Path, deferred: bool) {
    let message = if deferred {
        "更新任务已启动，当前 CLI 退出后会在后台完成安装。"
    } else {
        "YingDraft CLI 更新完成。"
    };

    output::emit_result(
        "update",
        message,
        UpdateSummary {
            repo: repo.to_string(),
            version: version.to_string(),
            asset: asset.to_string(),
            install_dir: install_dir.display().to_string(),
            deferred,
        },
    );
}

fn powershell_command() -> Result<&'static str> {
    if command_exists("pwsh") {
        Ok("pwsh")
    } else if command_exists("powershell") {
        Ok("powershell")
    } else {
        Err(anyhow!(
            "未找到 pwsh 或 powershell，无法在 Windows 上启动后台更新"
        ))
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("-NoProfile")
        .arg("-Command")
        .arg("$PSVersionTable.PSVersion | Out-Null")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn write_windows_handoff_script(
    script_path: &Path,
    current_pid: u32,
    installer: &Path,
    install_dir: &Path,
    temp_root: &Path,
) -> Result<()> {
    let script = format!(
        r#"$ErrorActionPreference = "Stop"
Wait-Process -Id {current_pid} -ErrorAction SilentlyContinue
& {installer} -InstallDir {install_dir}
$exitCode = $LASTEXITCODE
Start-Sleep -Seconds 1
Remove-Item -LiteralPath {temp_root} -Recurse -Force -ErrorAction SilentlyContinue
exit $exitCode
"#,
        installer = ps_single_quoted(installer),
        install_dir = ps_single_quoted(install_dir),
        temp_root = ps_single_quoted(temp_root),
    );

    let mut file = File::create(script_path)
        .with_context(|| format!("创建 Windows 更新脚本失败: {}", script_path.display()))?;
    file.write_all(script.as_bytes())
        .context("写入 Windows 更新脚本失败")?;
    Ok(())
}

fn ps_single_quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_release_assets() {
        assert_eq!(
            asset_name_for("macos", "aarch64").unwrap(),
            "yingdraft-macos-arm64.tar.gz"
        );
        assert_eq!(
            asset_name_for("macos", "x86_64").unwrap(),
            "yingdraft-macos-x64.tar.gz"
        );
        assert_eq!(
            asset_name_for("linux", "x86_64").unwrap(),
            "yingdraft-linux-x64.tar.gz"
        );
        assert_eq!(
            asset_name_for("windows", "x86_64").unwrap(),
            "yingdraft-windows-x64.tar.gz"
        );
    }

    #[test]
    fn builds_release_download_urls() {
        assert_eq!(
            release_download_url("owner/repo", "latest", "yingdraft-linux-x64.tar.gz"),
            "https://github.com/owner/repo/releases/latest/download/yingdraft-linux-x64.tar.gz"
        );
        assert_eq!(
            release_download_url("owner/repo", "v1.2.3", "yingdraft-linux-x64.tar.gz"),
            "https://github.com/owner/repo/releases/download/v1.2.3/yingdraft-linux-x64.tar.gz"
        );
    }
}
