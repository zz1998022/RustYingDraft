use anyhow::Result;
use serde_json::json;

use crate::output;

pub const VERSION: &str = env!("YINGDRAFT_VERSION");

pub fn run() -> Result<()> {
    output::emit_result(
        "version",
        &format!("YingDraft CLI {VERSION}"),
        json!({
            "version": VERSION,
        }),
    );
    Ok(())
}
