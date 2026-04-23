use anyhow::Result;
use camino::Utf8Path;
use jy_bundle::{import_bundle_with_progress, ImportBundleOptions};

use crate::output;

pub fn run(source: &Utf8Path, output_dir: &Utf8Path, name_override: Option<&str>) -> Result<()> {
    let summary = import_bundle_with_progress(
        &ImportBundleOptions {
            source: source.to_path_buf(),
            output: output_dir.to_path_buf(),
            name_override: name_override.map(str::to_string),
        },
        |event| {
            output::emit_progress("import-bundle", &event.stage, &event.message, &event.data);
        },
    )?;

    output::emit_result(
        "import-bundle",
        &format!("Imported bundle draft: {output_dir}"),
        &summary,
    );
    Ok(())
}
