mod commands;
mod output;

use clap::{error::ErrorKind, Parser, Subcommand, ValueEnum};
use serde_json::json;

/// 素材类型参数。
///
/// 用在模板替换等命令中，告诉 CLI 当前替换的是视频素材还是音频素材。
#[derive(Debug, Clone, Copy, ValueEnum)]
enum MediaTypeArg {
    Video,
    Audio,
}

/// 可编辑轨道类型参数。
///
/// 模板模式下，按“轨道种类 + 轨道名/轨道索引 + 片段索引”去定位某个待替换片段。
#[derive(Debug, Clone, Copy, ValueEnum)]
enum EditableTrackKindArg {
    Video,
    Audio,
    Text,
}

#[derive(Parser)]
#[command(name = "jy", about = "剪映草稿生成与模板处理 CLI")]
struct Cli {
    /// 控制 CLI 的输出格式。
    ///
    /// `text` 适合人手工执行；`json` 适合后端稳定解析。
    #[arg(long, value_enum, global = true, default_value_t = OutputFormatArg::Text)]
    output_format: OutputFormatArg,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormatArg {
    Text,
    Json,
}

impl From<OutputFormatArg> for output::OutputFormat {
    fn from(value: OutputFormatArg) -> Self {
        match value {
            OutputFormatArg::Text => output::OutputFormat::Text,
            OutputFormatArg::Json => output::OutputFormat::Json,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// 初始化一个空的 project manifest。
    Init {
        #[arg(short, long)]
        name: String,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 30)]
        fps: u32,
        #[arg(short, long)]
        output: camino::Utf8PathBuf,
    },
    /// 根据 project manifest 生成剪映草稿。
    Generate {
        #[arg(short, long)]
        project: camino::Utf8PathBuf,
        #[arg(short, long)]
        output: camino::Utf8PathBuf,
    },
    /// 根据本地视频、配音、BGM、SRT、水印生成一个可直接预览的 demo 草稿。
    GenerateDemo {
        #[arg(long)]
        name: String,
        #[arg(long)]
        video: camino::Utf8PathBuf,
        #[arg(long)]
        dubbing: camino::Utf8PathBuf,
        #[arg(long)]
        bgm: camino::Utf8PathBuf,
        #[arg(long)]
        subtitle: camino::Utf8PathBuf,
        #[arg(long)]
        watermark: camino::Utf8PathBuf,
        #[arg(short, long)]
        output: camino::Utf8PathBuf,
    },
    /// 将阿里云 VOD 时间轴 JSON 转换为剪映草稿。
    VodJsonToDraft {
        #[arg(long)]
        config: camino::Utf8PathBuf,
        #[arg(long)]
        assets_dir: Option<camino::Utf8PathBuf>,
        #[arg(short, long)]
        output: camino::Utf8PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// 查看草稿中的轨道、素材和模板可替换资源信息。
    Inspect {
        #[arg(short, long)]
        draft: camino::Utf8PathBuf,
    },
    /// 按素材名替换模板中的素材。
    TemplateReplaceMaterialName {
        #[arg(short, long)]
        draft: camino::Utf8PathBuf,
        #[arg(long)]
        target_name: String,
        #[arg(long)]
        media_type: MediaTypeArg,
        #[arg(long)]
        source: camino::Utf8PathBuf,
        #[arg(long)]
        material_name: Option<String>,
        #[arg(long, default_value_t = false)]
        replace_crop: bool,
        #[arg(short, long)]
        output: Option<camino::Utf8PathBuf>,
    },
    /// 按轨道和片段位置替换模板中的素材。
    TemplateReplaceMaterialSeg {
        #[arg(short, long)]
        draft: camino::Utf8PathBuf,
        #[arg(long)]
        track_kind: EditableTrackKindArg,
        #[arg(long)]
        track_name: Option<String>,
        #[arg(long)]
        track_index: Option<usize>,
        #[arg(long)]
        segment_index: usize,
        #[arg(long)]
        media_type: MediaTypeArg,
        #[arg(long)]
        source: camino::Utf8PathBuf,
        #[arg(long)]
        material_name: Option<String>,
        #[arg(long)]
        source_start: Option<String>,
        #[arg(long)]
        source_duration: Option<String>,
        #[arg(short, long)]
        output: Option<camino::Utf8PathBuf>,
    },
    /// 替换模板中的文本片段或多段文本模板。
    TemplateReplaceText {
        #[arg(short, long)]
        draft: camino::Utf8PathBuf,
        #[arg(long)]
        track_name: Option<String>,
        #[arg(long)]
        track_index: Option<usize>,
        #[arg(long)]
        segment_index: usize,
        #[arg(long, required = true)]
        text: Vec<String>,
        #[arg(long, default_value_t = true)]
        recalc_style: bool,
        #[arg(short, long)]
        output: Option<camino::Utf8PathBuf>,
    },
    /// 复制一份草稿目录，作为模板目标继续编辑。
    TemplateDuplicate {
        #[arg(long)]
        template_dir: camino::Utf8PathBuf,
        #[arg(long)]
        output_dir: camino::Utf8PathBuf,
        #[arg(long, default_value_t = false)]
        allow_replace: bool,
    },
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let wants_json = args.windows(2).any(|pair| {
        pair[0] == "--output-format" && pair[1].eq_ignore_ascii_case("json")
    }) || args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("--output-format=json"));

    output::init(if wants_json {
        output::OutputFormat::Json
    } else {
        output::OutputFormat::Text
    });

    // 先尝试解析 CLI；如果失败，JSON 模式下也输出稳定结构，而不是直接抛给 clap 默认渲染。
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if wants_json {
                match error.kind() {
                    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                        output::emit_result(
                            "cli",
                            "CLI help requested",
                            json!({ "text": error.to_string() }),
                        );
                        std::process::exit(0);
                    }
                    _ => output::emit_cli_parse_error(&error.to_string()),
                }
            } else {
                error.exit();
            }
            std::process::exit(2);
        }
    };

    output::init(cli.output_format.into());

    // 统一在这里分发所有命令，避免业务逻辑散落在 main 中。
    let (command_name, result) = match cli.command {
        Command::Init {
            name,
            width,
            height,
            fps,
            output,
        } => ("init", commands::init::run(&name, width, height, fps, &output)),
        Command::Generate { project, output } => {
            ("generate", commands::generate::run(&project, &output))
        }
        Command::GenerateDemo {
            name,
            video,
            dubbing,
            bgm,
            subtitle,
            watermark,
            output,
        } => (
            "generate-demo",
            commands::generate_demo::run(
                &name, &video, &dubbing, &bgm, &subtitle, &watermark, &output,
            ),
        ),
        Command::VodJsonToDraft {
            config,
            assets_dir,
            output,
            name,
        } => (
            "vod-json-to-draft",
            commands::vod_json_to_draft::run(
                &config,
                assets_dir.as_deref(),
                &output,
                name.as_deref(),
            ),
        ),
        Command::Inspect { draft } => ("inspect", commands::inspect::run(&draft)),
        Command::TemplateReplaceMaterialName {
            draft,
            target_name,
            media_type,
            source,
            material_name,
            replace_crop,
            output,
        } => (
            "template-replace-material-name",
            commands::template_replace_material_name::run(
                &draft,
                &target_name,
                media_type,
                &source,
                material_name.as_deref(),
                replace_crop,
                output.as_deref(),
            ),
        ),
        Command::TemplateReplaceMaterialSeg {
            draft,
            track_kind,
            track_name,
            track_index,
            segment_index,
            media_type,
            source,
            material_name,
            source_start,
            source_duration,
            output,
        } => (
            "template-replace-material-seg",
            commands::template_replace_material_seg::run(
                &draft,
                track_kind,
                track_name.as_deref(),
                track_index,
                segment_index,
                media_type,
                &source,
                material_name.as_deref(),
                source_start.as_deref(),
                source_duration.as_deref(),
                output.as_deref(),
            ),
        ),
        Command::TemplateReplaceText {
            draft,
            track_name,
            track_index,
            segment_index,
            text,
            recalc_style,
            output,
        } => (
            "template-replace-text",
            commands::template_replace_text::run(
                &draft,
                track_name.as_deref(),
                track_index,
                segment_index,
                &text,
                recalc_style,
                output.as_deref(),
            ),
        ),
        Command::TemplateDuplicate {
            template_dir,
            output_dir,
            allow_replace,
        } => (
            "template-duplicate",
            commands::template_duplicate::run(&template_dir, &output_dir, allow_replace),
        ),
    };

    if let Err(error) = result {
        output::emit_error(Some(command_name), &error);
        std::process::exit(1);
    }
}
