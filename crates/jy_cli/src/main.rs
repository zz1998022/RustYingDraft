mod commands;

use clap::{Parser, ValueEnum};

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
enum Cli {
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

fn main() -> anyhow::Result<()> {
    // 统一在这里分发所有命令，避免业务逻辑散落在 main 中。
    let cli = Cli::parse();
    match cli {
        Cli::Init {
            name,
            width,
            height,
            fps,
            output,
        } => commands::init::run(&name, width, height, fps, &output),
        Cli::Generate { project, output } => commands::generate::run(&project, &output),
        Cli::GenerateDemo {
            name,
            video,
            dubbing,
            bgm,
            subtitle,
            watermark,
            output,
        } => commands::generate_demo::run(
            &name, &video, &dubbing, &bgm, &subtitle, &watermark, &output,
        ),
        Cli::VodJsonToDraft {
            config,
            assets_dir,
            output,
            name,
        } => commands::vod_json_to_draft::run(
            &config,
            assets_dir.as_deref(),
            &output,
            name.as_deref(),
        ),
        Cli::Inspect { draft } => commands::inspect::run(&draft),
        Cli::TemplateReplaceMaterialName {
            draft,
            target_name,
            media_type,
            source,
            material_name,
            replace_crop,
            output,
        } => commands::template_replace_material_name::run(
            &draft,
            &target_name,
            media_type,
            &source,
            material_name.as_deref(),
            replace_crop,
            output.as_deref(),
        ),
        Cli::TemplateReplaceMaterialSeg {
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
        } => commands::template_replace_material_seg::run(
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
        Cli::TemplateReplaceText {
            draft,
            track_name,
            track_index,
            segment_index,
            text,
            recalc_style,
            output,
        } => commands::template_replace_text::run(
            &draft,
            track_name.as_deref(),
            track_index,
            segment_index,
            &text,
            recalc_style,
            output.as_deref(),
        ),
        Cli::TemplateDuplicate {
            template_dir,
            output_dir,
            allow_replace,
        } => commands::template_duplicate::run(&template_dir, &output_dir, allow_replace),
    }
}
