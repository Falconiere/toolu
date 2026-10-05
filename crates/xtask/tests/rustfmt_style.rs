//! The root `rustfmt.toml` enforces two spaces per indentation level and no tabs:
//! real `rustfmt --check` runs over sources written to a temp directory.

use std::error::Error;
use std::fs;
use std::process::Command;

type TestResult = Result<(), Box<dyn Error>>;

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../rustfmt.toml");

/// Whether `rustfmt --check` with the root config accepts `source` unchanged.
fn accepted(source: &str) -> Result<bool, Box<dyn Error>> {
  let dir = tempfile::tempdir()?;
  let file = dir.path().join("sample.rs");
  fs::write(&file, source)?;
  let output = Command::new("rustfmt")
    .args(["--check", "--config-path", CONFIG])
    .arg(&file)
    .output()?;
  match output.status.code() {
    Some(0) => Ok(true),
    Some(1) => Ok(false),
    _ => Err(
      format!(
        "rustfmt failed: {}",
        String::from_utf8_lossy(&output.stderr)
      )
      .into(),
    ),
  }
}

#[test]
fn four_spaces_for_one_level_fails() -> TestResult {
  assert!(!accepted("fn main() {\n    let _x = 1;\n}\n")?);
  Ok(())
}

#[test]
fn a_tab_indentation_fails() -> TestResult {
  assert!(!accepted("fn main() {\n\tlet _x = 1;\n}\n")?);
  Ok(())
}

#[test]
fn two_spaces_per_level_pass_at_two_levels() -> TestResult {
  assert!(accepted(
    "fn main() {\n  if true {\n    let _x = 1;\n  }\n}\n"
  )?);
  Ok(())
}
