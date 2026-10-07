use std::fs;

use super::{expand, repo_relative};

#[test]
fn expansion_uses_real_files_in_sorted_order_and_keeps_literal_fallback() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(dir.path().join(".env"), "secret\n").expect("write .env");
  fs::write(dir.path().join(".enx"), "other\n").expect("write .enx");
  assert_eq!(
    expand(".en[vx]", dir.path()),
    vec![".env", ".enx", ".en[vx]"]
  );
  assert_eq!(expand("missing*", dir.path()), vec!["missing*"]);
  assert_eq!(expand("*", dir.path()), vec!["*"]);
}

#[test]
fn only_an_inside_absolute_path_becomes_relative() {
  let dir = tempfile::tempdir().expect("tempdir");
  let root = dir.path().join("project");
  assert_eq!(
    repo_relative(&format!("{}/src/a.rs", root.display()), &root),
    "src/a.rs"
  );
  assert_eq!(repo_relative("/outside/a.rs", &root), "/outside/a.rs");
}
