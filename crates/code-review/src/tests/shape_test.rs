use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;
use toolu_runtime::env::Env;

use super::{ShapeError, resolve_merge_base, shape};

pub(super) fn git(root: &Path, args: &[&str]) -> String {
  let out = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    // Default global ignore and attributes files would otherwise leak into fixtures.
    .env("XDG_CONFIG_HOME", "/dev/null")
    .output()
    .expect("git starts");
  assert!(
    out.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8(out.stdout).expect("git text")
}

pub(super) fn isolated_env() -> Env {
  Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null")
}

pub(super) fn repo() -> TempDir {
  let dir = tempfile::tempdir().expect("temp repo");
  git(dir.path(), &["init", "-b", "main"]);
  git(dir.path(), &["config", "user.name", "Test"]);
  git(dir.path(), &["config", "user.email", "test@example.com"]);
  fs::write(
    dir.path().join("readme.txt"),
    "one\ntwo\nthree\nfour\nfive\nsix\n",
  )
  .expect("base file");
  fs::write(
    dir.path().join(".gitattributes"),
    "attr.rs linguist-generated\n",
  )
  .expect("attributes");
  commit(dir.path());
  git(dir.path(), &["switch", "-c", "feature"]);
  dir
}

pub(super) fn commit(root: &Path) {
  git(root, &["add", "-A"]);
  git(root, &["commit", "-m", "fixture"]);
}

#[test]
fn shape_base_uses_gate_hash_and_reports_empty_diff() {
  let dir = repo();
  let env = isolated_env();
  assert!(matches!(
    shape(&env, dir.path(), Some("main")),
    Err(ShapeError::EmptyDiff)
  ));
  fs::write(dir.path().join("readme.txt"), "base\nnext\n").expect("change");
  commit(dir.path());
  let shaped = shape(&env, dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.base_ref, "main");
  assert_eq!(shaped.gate_files, vec!["readme.txt"]);
  assert_eq!(
    shaped.diff_sha,
    toolu_state::diff_sha::diff_sha(&env, dir.path(), "main").expect("gate hash")
  );
  assert!(!shaped.uncommitted);
  fs::write(dir.path().join("readme.txt"), "working tree only\n").expect("working change");
  let with_worktree = shape(&env, dir.path(), Some("main")).expect("committed shape");
  assert!(with_worktree.uncommitted);
  assert_eq!(with_worktree.diff_sha, shaped.diff_sha);
  assert!(!with_worktree.files[0].patch.contains("working tree only"));
}

#[test]
fn shape_base_reports_unrelated_history() {
  let dir = repo();
  git(dir.path(), &["switch", "--orphan", "orphan"]);
  fs::write(dir.path().join("other.txt"), "other\n").expect("other file");
  commit(dir.path());
  assert!(matches!(
    shape(&isolated_env(), dir.path(), Some("main")),
    Err(ShapeError::NoMergeBase { .. })
  ));
}

#[test]
fn shape_base_reports_git_merge_failure_as_git_error() {
  let dir = repo();
  assert!(matches!(
    resolve_merge_base(&isolated_env(), dir.path(), "invalid-ref"),
    Err(ShapeError::Git(_))
  ));
}

#[test]
fn shape_base_reports_missing_shallow_history() {
  let source = repo();
  fs::write(source.path().join("readme.txt"), "feature\n").expect("feature");
  commit(source.path());
  let destination = tempfile::tempdir().expect("clone parent");
  let url = format!("file://{}", source.path().display());
  git(
    destination.path(),
    &[
      "clone",
      "--depth=1",
      "--branch",
      "feature",
      &url,
      "checkout",
    ],
  );
  let checkout = destination.path().join("checkout");
  git(
    &checkout,
    &[
      "fetch",
      "--depth=1",
      "origin",
      "main:refs/remotes/origin/main",
    ],
  );
  assert!(matches!(
    shape(&isolated_env(), &checkout, Some("origin/main")),
    Err(ShapeError::NoMergeBase { shallow: true })
  ));
}

#[test]
fn shape_anchor_rename_with_edits_uses_real_lines() {
  let dir = repo();
  fs::create_dir(dir.path().join("docs")).expect("docs");
  git(dir.path(), &["mv", "readme.txt", "docs/readme.txt"]);
  fs::write(
    dir.path().join("docs/readme.txt"),
    "one\nTWO\nthree\nfour\nfive\nsix\nseven\n",
  )
  .expect("rename edits");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape rename");
  let actual = serde_json::json!({
    "files": shaped.files.iter().map(|file| serde_json::json!({
      "path": file.path,
      "old_path": file.old_path,
      "kind": file.kind,
      "lines": file.lines
    })).collect::<Vec<_>>(),
    "renames": shaped.renames,
    "coverage": shaped.coverage
  });
  let golden: serde_json::Value =
    serde_json::from_str(include_str!("../../tests/fixtures/shape/rename.json")).expect("golden");
  assert_eq!(actual, golden);
}

#[test]
fn shape_anchor_deleted_file_uses_base_lines() {
  let dir = repo();
  git(dir.path(), &["rm", "readme.txt"]);
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape deletion");
  let actual = serde_json::json!({
    "files": shaped.files.iter().map(|file| serde_json::json!({
      "path": file.path,
      "old_path": file.old_path,
      "kind": file.kind,
      "lines": file.lines
    })).collect::<Vec<_>>(),
    "coverage": shaped.coverage
  });
  let golden: serde_json::Value =
    serde_json::from_str(include_str!("../../tests/fixtures/shape/deleted.json")).expect("golden");
  assert_eq!(actual, golden);
}

#[test]
fn shape_coverage_skips_every_noise_path() {
  let dir = repo();
  fs::create_dir(dir.path().join("vendor")).expect("vendor");
  fs::create_dir(dir.path().join("target")).expect("target");
  fs::write(dir.path().join("Cargo.lock"), "lock\n").expect("lock");
  fs::write(dir.path().join("vendor/lib.rs"), "vendored\n").expect("vendor file");
  fs::write(dir.path().join("target/output"), "output\n").expect("build file");
  fs::write(dir.path().join("gen.rs"), "// @generated\ncode\n").expect("marker");
  fs::write(dir.path().join("attr.rs"), "code\n").expect("attr");
  fs::write(dir.path().join("binary.bin"), [0, 1, 2, 3]).expect("binary");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape noise");
  let actual = serde_json::json!({
    "coverage": shaped.coverage,
    "review_lines": shaped.files.iter().map(|file| file.lines.len()).sum::<usize>()
  });
  let golden: serde_json::Value =
    serde_json::from_str(include_str!("../../tests/fixtures/shape/noise.json")).expect("golden");
  assert_eq!(actual, golden);
}

#[test]
fn shape_coverage_partitions_gate_paths_and_marks_large_file_unreviewed() {
  let dir = repo();
  fs::write(
    dir.path().join("readme.txt"),
    "one\nTWO\nthree\nfour\nfive\nsix\n",
  )
  .expect("edit");
  fs::write(dir.path().join("Cargo.lock"), "lock\n").expect("lock");
  fs::write(dir.path().join("large.txt"), "x\n".repeat(70_000)).expect("large");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  let mut covered = shaped.coverage.reviewed.clone();
  covered.extend(shaped.coverage.skipped.iter().map(|item| item.path.clone()));
  covered.extend(
    shaped
      .coverage
      .unreviewed
      .iter()
      .map(|item| item.path.clone()),
  );
  covered.sort();
  assert_eq!(covered, shaped.gate_files);
  assert_eq!(shaped.coverage.unreviewed.len(), 1);
  assert_eq!(shaped.coverage.unreviewed[0].path, "large.txt");
  assert!(
    shaped
      .chunks
      .iter()
      .all(|chunk| chunk.bytes <= crate::budget::DIFF_CHUNK_MAX_BYTES)
  );
  assert_eq!(shaped.files[0].lines.len(), 2);
}

#[test]
fn shape_coverage_chunks_complete_files() {
  let dir = repo();
  fs::write(dir.path().join("alpha.txt"), "a\n".repeat(36_000)).expect("alpha");
  fs::write(dir.path().join("beta.txt"), "b\n".repeat(36_000)).expect("beta");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.files.len(), 2);
  assert_eq!(shaped.chunks.len(), 2);
  assert_eq!(shaped.chunks[0].files, vec![0]);
  assert_eq!(shaped.chunks[1].files, vec![1]);
}

#[test]
fn shape_coverage_reviews_marker_text_and_leaves_non_utf8_text_unreviewed() {
  let dir = repo();
  fs::write(
    dir.path().join("notes.rs"),
    "fn a() {}\n// Binary files a and b differ\n\n\n\n\n// Generated by x; do not edit.\n",
  )
  .expect("marker text");
  fs::write(dir.path().join("latin1.txt"), b"caf\xe9\n").expect("latin-1 text");
  fs::write(dir.path().join("large.bin"), vec![0u8; 9 * 1024 * 1024]).expect("large binary");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, vec!["notes.rs"]);
  assert_eq!(shaped.files[0].lines.len(), 7);
  let skipped: Vec<_> = shaped
    .coverage
    .skipped
    .iter()
    .map(|item| (item.path.as_str(), item.reason.as_str()))
    .collect();
  assert_eq!(skipped, [("large.bin", "binary")]);
  assert_eq!(shaped.coverage.unreviewed.len(), 1);
  assert_eq!(shaped.coverage.unreviewed[0].path, "latin1.txt");
}

#[test]
fn shape_coverage_empty_added_file_is_explicitly_unreviewed() {
  let dir = repo();
  fs::write(dir.path().join("empty.txt"), "").expect("empty file");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, Vec::<String>::new());
  assert_eq!(shaped.coverage.unreviewed.len(), 1);
  assert_eq!(shaped.coverage.unreviewed[0].path, "empty.txt");
}
