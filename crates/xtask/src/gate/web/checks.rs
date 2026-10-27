//! Native checks for asset budgets and logic-module test coverage.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::CONFIG;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Rules {
  max_code_lines: usize,
  max_served_bytes: u64,
  max_font_bytes: u64,
}

/// Report native size and test-pair findings for one asset tree.
pub(super) fn check(root: &Path, asset: &Path) -> Result<Vec<String>, String> {
  let path = root.join(CONFIG).join("rules.json");
  let text = std::fs::read_to_string(&path)
    .map_err(|err| format!("web: cannot read {}: {err}", path.display()))?;
  let rules: Rules =
    serde_json::from_str(&text).map_err(|err| format!("web: {}: {err}", path.display()))?;
  let mut files = Vec::new();
  visit(asset, &mut files)?;
  let mut found = Vec::new();
  let (mut served, mut fonts) = (0_u64, 0_u64);
  for file in &files {
    let rel = file.strip_prefix(asset).map_err(|err| err.to_string())?;
    let ext = file.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let bytes = file
      .metadata()
      .map_err(|err| format!("{}: {err}", file.display()))?
      .len();
    if rel.starts_with("fonts") && ext != "txt" {
      fonts = fonts.saturating_add(bytes);
    }
    if ["html", "css", "js"].contains(&ext) && !rel.starts_with("tests") {
      served = served.saturating_add(bytes);
    }
    if ["html", "css"].contains(&ext) {
      check_lines(file, ext, rules.max_code_lines, &mut found)?;
    }
    if ext == "js" && rel.starts_with("js/logic") {
      check_test(asset, file, &mut found);
    }
  }
  if served > rules.max_served_bytes {
    found.push(format!(
      "{}:1: max-served-bytes: {served} > {}",
      asset.display(),
      rules.max_served_bytes
    ));
  }
  if fonts > rules.max_font_bytes {
    found.push(format!(
      "{}:1: max-font-bytes: {fonts} > {}",
      asset.join("fonts").display(),
      rules.max_font_bytes
    ));
  }
  Ok(found)
}

fn visit(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
  let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  for entry in entries {
    let entry = entry.map_err(|err| format!("{}: {err}", dir.display()))?;
    let kind = entry
      .file_type()
      .map_err(|err| format!("{}: {err}", entry.path().display()))?;
    if kind.is_symlink() {
      return Err(format!(
        "web: symlink in asset directory: {}",
        entry.path().display()
      ));
    }
    if kind.is_dir() {
      visit(&entry.path(), files)?;
    } else if kind.is_file() {
      files.push(entry.path());
    }
  }
  files.sort();
  Ok(())
}

fn check_lines(file: &Path, ext: &str, max: usize, found: &mut Vec<String>) -> Result<(), String> {
  let text = std::fs::read_to_string(file)
    .map_err(|err| format!("web: cannot read {}: {err}", file.display()))?;
  let count = code_lines(&text, ext);
  if count > max {
    found.push(format!(
      "{}:{}: max-code-lines: {count} > {max}",
      file.display(),
      text.lines().count()
    ));
  }
  Ok(())
}

fn check_test(asset: &Path, file: &Path, found: &mut Vec<String>) {
  let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
    return;
  };
  if !asset
    .join("tests")
    .join(format!("{stem}.test.js"))
    .is_file()
  {
    found.push(format!(
      "{}:1: logic-test-pair: tests/{stem}.test.js is missing",
      file.display()
    ));
  }
}

fn code_lines(text: &str, ext: &str) -> usize {
  let (open, close): (&[u8], &[u8]) = if ext == "html" {
    (b"<!--", b"-->")
  } else {
    (b"/*", b"*/")
  };
  let mut inside = false;
  text
    .lines()
    .filter(|line| {
      let bytes = line.as_bytes();
      let (mut index, mut code) = (0, false);
      while index < bytes.len() {
        if inside && bytes[index..].starts_with(close) {
          inside = false;
          index += close.len();
        } else if inside {
          index += 1;
        } else if bytes[index..].starts_with(open) {
          inside = true;
          index += open.len();
        } else {
          code |= !bytes[index].is_ascii_whitespace();
          index += 1;
        }
      }
      code
    })
    .count()
}

#[cfg(test)]
#[path = "tests/checks_test.rs"]
mod tests;
