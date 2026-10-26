use std::path::Path;
use std::process::{Command, Stdio};

use super::{CLONE, DOCKERFILE, SCRIPT, bundle, channel, run};
use crate::deploy_guard::RECEIPT;
use crate::options::Options;
use crate::tests::git;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// A committed repository with `origin/main` at `HEAD`, a stale receipt, and its `HEAD`.
fn repo() -> (tempfile::TempDir, String) {
  let dir = tempfile::tempdir().unwrap();
  git(dir.path(), &["init", "-q"]);
  std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
  std::fs::copy(
    format!("{ROOT}/rust-toolchain.toml"),
    dir.path().join("rust-toolchain.toml"),
  )
  .unwrap();
  git(dir.path(), &["add", "."]);
  git(dir.path(), &["commit", "-qm", "probe"]);
  git(
    dir.path(),
    &["update-ref", "refs/remotes/origin/main", "HEAD"],
  );
  let receipt = dir.path().join(RECEIPT);
  std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
  std::fs::write(&receipt, "{\"commit\":\"stale\"}").unwrap();
  let head = git(dir.path(), &["rev-parse", "HEAD"]);
  (dir, head)
}

fn options(dir: &Path) -> Options {
  Options {
    root: dir.to_path_buf(),
    ..Options::default()
  }
}

#[test]
fn a_dirty_tree_is_refused_and_the_old_receipt_is_gone() {
  let (dir, _) = repo();
  std::fs::write(dir.path().join(".gitignore"), "edited\n").unwrap();
  let error = run(&options(dir.path())).unwrap_err();
  assert!(error.contains("dirty"), "{error}");
  assert!(!dir.path().join(RECEIPT).exists());
}

#[test]
fn a_missing_origin_main_is_refused_and_the_old_receipt_is_gone() {
  let (dir, _) = repo();
  git(
    dir.path(),
    &["update-ref", "-d", "refs/remotes/origin/main"],
  );
  let error = run(&options(dir.path())).unwrap_err();
  assert!(error.contains("refs/remotes/origin/main"), "{error}");
  assert!(!dir.path().join(RECEIPT).exists());
}

#[test]
fn the_image_pins_match_the_release_workflow_and_toolchain() {
  let workflow =
    std::fs::read_to_string(format!("{ROOT}/.github/workflows/release-native.yml")).unwrap();
  let (_, tools) = workflow.split_once("tool: ").unwrap();
  let (_, ast_grep) = workflow.split_once("npm install -g ").unwrap();
  let pins: Vec<&str> = tools.lines().next().unwrap().split(',').collect();
  assert_eq!(pins.len(), 4, "{pins:?}");
  for pin in pins.into_iter().chain(ast_grep.lines().next()) {
    assert!(
      DOCKERFILE.contains(pin.trim()),
      "the Dockerfile lacks {pin}"
    );
  }
  let toolchain = std::fs::read_to_string(format!("{ROOT}/rust-toolchain.toml")).unwrap();
  let channel = channel(Path::new(ROOT)).unwrap();
  assert!(
    toolchain.contains(&format!("channel = \"{channel}\"")),
    "{channel}"
  );
}

#[test]
fn the_container_script_runs_the_full_verify_job() {
  for step in [
    "npm ci --ignore-scripts",
    "python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'",
    "cargo xtask gate",
  ] {
    assert!(SCRIPT.contains(step), "the script lacks {step}");
  }
  assert!(!SCRIPT.contains("--only"));
}

/// Run the clone stage on the host, as the container does, with `bundle` on stdin.
fn clone(bundle: &Path, expected: &str) -> (bool, tempfile::TempDir) {
  let parent = tempfile::tempdir().unwrap();
  let status = Command::new("sh")
    .args(["-euc", CLONE])
    .env("SRC", parent.path().join("src"))
    .env("EXPECTED", expected)
    .stdin(Stdio::from(std::fs::File::open(bundle).unwrap()))
    .stdout(Stdio::null())
    .status()
    .unwrap();
  (status.success(), parent)
}

#[test]
fn the_clone_stage_checks_out_head_with_origin_main_and_refuses_another_sha() {
  let (dir, head) = repo();
  let file = bundle(dir.path()).unwrap();
  let (ok, parent) = clone(&file, &head);
  assert!(ok);
  let src = parent.path().join("src");
  assert_eq!(git(&src, &["rev-parse", "HEAD"]), head);
  assert_eq!(git(&src, &["rev-parse", "refs/remotes/origin/main"]), head);
  let (ok, _) = clone(&file, &"0".repeat(40));
  assert!(
    !ok,
    "a bundle whose HEAD is not the expected SHA was accepted"
  );
}
