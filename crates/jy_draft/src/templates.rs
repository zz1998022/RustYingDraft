use serde_json::Value;

const DRAFT_CONTENT_TEMPLATE: &str = include_str!("../templates/draft_content_template.json");
const DRAFT_META_TEMPLATE: &str = include_str!("../templates/draft_meta_info.json");

/// Load the draft_content.json template as a mutable Value.
pub fn load_content_template() -> Result<Value, serde_json::Error> {
    serde_json::from_str(DRAFT_CONTENT_TEMPLATE)
}

/// Load the draft_meta_info.json template as a mutable Value.
pub fn load_meta_template() -> Result<Value, serde_json::Error> {
    serde_json::from_str(DRAFT_META_TEMPLATE)
}
