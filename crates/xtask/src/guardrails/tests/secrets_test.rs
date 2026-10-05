use super::check;
use crate::guardrails::tests::{context, tree};

/// A GitHub-shaped token assembled at runtime, so no committed file holds one.
fn token() -> String {
  format!("ghp_{}", "a1".repeat(18))
}

#[test]
fn a_secret_in_any_file_under_crates_is_found_with_its_line() {
  let readme = format!("# demo\n\ntoken: {}\n", token());
  let tree = tree(&[
    ("crates/demo/README.md", &readme),
    ("docs/notes.md", &readme),
  ]);
  let found = check(&context(&tree.workspace)).unwrap();
  assert_eq!(found.len(), 1);
  assert_eq!(
    (found[0].path.as_str(), found[0].line),
    ("crates/demo/README.md", 3)
  );
  assert_eq!(
    found[0].message,
    "looks like a committed secret (github-token) — remove it and rotate it"
  );
}

#[test]
fn binary_files_are_skipped_and_a_bad_pattern_is_a_setup_error() {
  let tree = tree(&[("crates/demo/blob.bin", "\u{0}")]);
  std::fs::write(
    tree.workspace.root.join("crates/demo/blob.bin"),
    [0xff_u8, 0xfe],
  )
  .unwrap();
  let mut ctx = context(&tree.workspace);
  assert_eq!(check(&ctx).unwrap(), Vec::new());
  ctx.rules.secrets[0].regex = "(".to_owned();
  assert!(
    check(&ctx)
      .unwrap_err()
      .starts_with("rules.json secret aws-access-key:")
  );
}
