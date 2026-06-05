use anyhow::{bail, Context, Result};
use camino::Utf8Path;

use crate::fs_util::validate_simple_relative_path;

pub(crate) fn parse_concat_file(path: &Utf8Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read concat file: {path}"))?;
    let mut files = Vec::new();
    for (line_index, raw_line) in content.lines().enumerate() {
        let line_no = line_index + 1;
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("file ") else {
            bail!("concat line {line_no} only supports file entries");
        };
        let relative = parse_concat_file_value(rest)
            .with_context(|| format!("invalid concat line {line_no}"))?;
        validate_simple_relative_path(&format!("concat line {line_no} path"), &relative)?;
        files.push(relative);
    }

    if files.is_empty() {
        bail!("concat file must contain at least one file entry");
    }
    Ok(files)
}

fn parse_concat_file_value(value: &str) -> Result<String> {
    let value = value.trim();
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Ok(value[1..value.len() - 1].to_string());
    }
    bail!("concat file entry must use single quotes");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_concat_file_rejects_unsupported_lines() -> Result<()> {
        let temp = tempdir()?;
        let concat_path = temp.path().join("concat.txt");
        fs::write(&concat_path, "duration 2\n")?;
        let concat_path = camino::Utf8PathBuf::from_path_buf(concat_path).unwrap();
        let error = parse_concat_file(&concat_path).unwrap_err();
        assert!(format!("{error:#}").contains("only supports file entries"));
        Ok(())
    }
}
