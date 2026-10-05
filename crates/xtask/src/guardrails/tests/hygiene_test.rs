use super::check;
use crate::guardrails::tests::{MANIFEST, context, rules, tree};

#[test]
fn an_inheriting_documented_crate_passes() {
  let fine = tree(&[
    ("crates/demo/Cargo.toml", MANIFEST),
    ("crates/demo/src/lib.rs", "//! Demo.\n"),
  ]);
  assert_eq!(check(&context(&fine.workspace)).unwrap(), Vec::new());
}

#[test]
fn missing_lints_version_and_crate_doc_are_findings() {
  let bare = tree(&[
    (
      "crates/demo/Cargo.toml",
      "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[lints.clippy]\nall = \"warn\"\n",
    ),
    (
      "crates/demo/src/lib.rs",
      "/// Not a crate doc.\npub fn f() {}\n",
    ),
    ("crates/demo/src/main.rs", "fn main() {}\n"),
  ]);
  let found = check(&context(&bare.workspace)).unwrap();
  assert_eq!(rules(&found), ["crate-hygiene"; 4]);
  let messages: Vec<&str> = found.iter().map(|f| f.message.as_str()).collect();
  assert_eq!(
    messages,
    [
      "missing `[lints] workspace = true`",
      "missing `version.workspace = true`",
      "no crate-level `//!` doc comment",
      "no crate-level `//!` doc comment",
    ]
  );
}

#[test]
fn an_unreadable_or_invalid_manifest_is_a_setup_error() {
  let missing = tree(&[]);
  assert!(
    check(&context(&missing.workspace))
      .unwrap_err()
      .contains("Cargo.toml")
  );
  let invalid = tree(&[("crates/demo/Cargo.toml", "[package\n")]);
  assert!(
    check(&context(&invalid.workspace))
      .unwrap_err()
      .starts_with("crates/demo/Cargo.toml:")
  );
}
