use std::io::{self, Write};
use std::sync::OnceLock;

use anyhow::Error;
use serde::Serialize;
use serde_json::{json, Value};

/// CLI 的统一输出格式。
///
/// - `Text` 面向人类终端，强调可读性
/// - `Json` 面向后端或脚本，输出稳定的 JSON 事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// 全局输出配置。
///
/// 当前 CLI 是单进程、单任务执行模型，用 `OnceLock` 保存一次初始化后的输出模式，
/// 可以避免把格式参数层层传递到所有命令函数里。
#[derive(Debug, Clone, Copy)]
struct OutputConfig {
    format: OutputFormat,
}

static OUTPUT_CONFIG: OnceLock<OutputConfig> = OnceLock::new();

/// 初始化输出配置。
pub fn init(format: OutputFormat) {
    let _ = OUTPUT_CONFIG.set(OutputConfig { format });
}

/// 判断当前是否启用了 JSON 输出模式。
pub fn is_json() -> bool {
    matches!(current_format(), OutputFormat::Json)
}

/// 输出一个过程事件。
///
/// 这个接口主要给“有中间进度”的命令使用，例如远程素材下载。
/// JSON 模式下会输出一行独立事件，便于后端按行流式消费。
pub fn emit_progress<T: Serialize>(command: &str, stage: &str, message: &str, data: T) {
    match current_format() {
        OutputFormat::Text => println!("{message}"),
        OutputFormat::Json => emit_json(json!({
            "kind": "progress",
            "ok": true,
            "command": command,
            "stage": stage,
            "message": message,
            "data": data,
        })),
    }
}

/// 输出最终成功结果。
pub fn emit_result<T: Serialize>(command: &str, message: &str, data: T) {
    match current_format() {
        OutputFormat::Text => println!("{message}"),
        OutputFormat::Json => emit_json(json!({
            "kind": "result",
            "ok": true,
            "command": command,
            "message": message,
            "data": data,
        })),
    }
}

/// 输出最终失败结果。
///
/// 这里会尽量把常见错误归类成稳定的 `code`，方便 Java/Go/Node 这类调用方
/// 不必依赖模糊的英文报错文本去做判断。
pub fn emit_error(command: Option<&str>, error: &Error) {
    let code = classify_error(error);
    let command = command.unwrap_or("cli");
    let causes = error.chain().map(ToString::to_string).collect::<Vec<_>>();

    match current_format() {
        OutputFormat::Text => {
            eprintln!("Error [{code}] {}", error);
            for cause in causes.iter().skip(1) {
                eprintln!("Caused by: {cause}");
            }
        }
        OutputFormat::Json => emit_json(json!({
            "kind": "error",
            "ok": false,
            "command": command,
            "code": code,
            "message": error.to_string(),
            "causes": causes,
        })),
    }
}

/// 输出 CLI 参数解析错误。
pub fn emit_cli_parse_error(message: &str) {
    match current_format() {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => emit_json(json!({
            "kind": "error",
            "ok": false,
            "command": "cli",
            "code": "invalid_args",
            "message": message,
        })),
    }
}

fn current_format() -> OutputFormat {
    OUTPUT_CONFIG
        .get()
        .map(|config| config.format)
        .unwrap_or(OutputFormat::Text)
}

/// 将事件稳定地输出为单行 JSON。
///
/// 单行格式更适合后端逐行读取；即使有进度事件，也不需要等待整段文本结束。
fn emit_json(value: Value) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, &value).expect("failed to serialize CLI json output");
    writeln!(&mut handle).expect("failed to write CLI json output");
    handle.flush().expect("failed to flush CLI json output");
}

fn classify_error(error: &Error) -> &'static str {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<std::io::Error>() {
            return match io_error.kind() {
                std::io::ErrorKind::NotFound => "input_not_found",
                std::io::ErrorKind::PermissionDenied => "permission_denied",
                std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                    "invalid_input"
                }
                _ => "io_error",
            };
        }

        if cause.downcast_ref::<serde_json::Error>().is_some() {
            return "input_parse_failed";
        }

        if let Some(http_error) = cause.downcast_ref::<reqwest::Error>() {
            return if http_error.is_timeout() {
                "download_timeout"
            } else {
                "download_failed"
            };
        }
    }

    let message = error.to_string().to_ascii_lowercase();
    if message.contains("ffprobe") {
        "ffprobe_not_found"
    } else if message.contains("subtitle") && message.contains("parse") {
        "subtitle_parse_failed"
    } else if message.contains("download") {
        "download_failed"
    } else if message.contains("write") || message.contains("draft") {
        "draft_write_failed"
    } else {
        "command_failed"
    }
}
