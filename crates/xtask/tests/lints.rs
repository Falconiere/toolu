//! The workspace lints deny `unwrap`, `expect` and `panic!` outside tests: real
//! `cargo clippy` runs over a temp workspace built from the repo's own root
//! `Cargo.toml` and `clippy.toml`, with one probe crate as its only member.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn Error>>;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The repo's root manifest with `members` replaced by the probe crate.
fn probe_manifest() -> Result<String, Box<dyn Error>> {
  let manifest = fs::read_to_string(Path::new(ROOT).join("Cargo.toml"))?;
  let start = manifest
    .find("members = [")
    .ok_or("no members list in Cargo.toml")?;
  let end = start
    + manifest[start..]
      .find(']')
      .ok_or("unterminated members list")?;
  Ok(format!(
    "{}members = [\"probe\"]{}",
    &manifest[..start],
    &manifest[end + 1..]
  ))
}

/// `cargo clippy --all-targets -- -D warnings`, as CI runs it, over a probe `lib.rs`.
fn clippy(lib: &str) -> Result<Output, Box<dyn Error>> {
  let dir = tempfile::tempdir()?;
  let root = dir.path();
  fs::write(root.join("Cargo.toml"), probe_manifest()?)?;
  fs::copy(
    Path::new(ROOT).join("clippy.toml"),
    root.join("clippy.toml"),
  )?;
  fs::create_dir_all(root.join("probe/src"))?;
  fs::write(
    root.join("probe/Cargo.toml"),
    "[package]\nname = \"probe\"\nversion.workspace = true\nedition.workspace = true\n\n[lints]\nworkspace = true\n",
  )?;
  fs::write(root.join("probe/src/lib.rs"), lib)?;
  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  Ok(
    Command::new(cargo)
      .args([
        "clippy",
        "--offline",
        "--all-targets",
        "--",
        "-D",
        "warnings",
      ])
      .current_dir(root)
      .env("CARGO_TARGET_DIR", root.join("target"))
      .output()?,
  )
}

#[test]
fn unwrap_expect_and_panic_outside_tests_fail() -> TestResult {
  let output = clippy(
    "pub fn first(x: Option<u8>) -> u8 {\n  x.unwrap()\n}\n\n\
     pub fn second(x: Option<u8>) -> u8 {\n  x.expect(\"set\")\n}\n\n\
     pub fn third() {\n  panic!(\"no\");\n}\n",
  )?;
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(!output.status.success(), "{stderr}");
  // clippy names each lint in its documentation link.
  for lint in [
    "index.html#unwrap_used",
    "index.html#expect_used",
    "index.html#panic",
  ] {
    assert!(stderr.contains(lint), "{lint} missing from:\n{stderr}");
  }
  Ok(())
}

#[test]
fn unwrap_inside_tests_passes() -> TestResult {
  let output = clippy(
    "pub fn id(x: Option<u8>) -> Option<u8> {\n  x\n}\n\n\
     #[cfg(test)]\nmod tests {\n  #[test]\n  fn id_keeps_the_value() {\n    \
     assert_eq!(super::id(Some(1)).unwrap(), 1);\n  }\n}\n",
  )?;
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  Ok(())
}
