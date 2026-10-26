use std::path::Path;
use std::process::{Command, Stdio};

use super::{
  CLONE, DOCKERFILE, SCRIPT, build_command, bundle, channel, container, run, with_input,
};
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
  // Tests run the real npx (the golden `bunx` shim), jq and gh, as on the CI runner image.
  for tool in ["/usr/local/bin/npx", "--no-install-recommends jq gh"] {
    assert!(DOCKERFILE.contains(tool), "the image lacks {tool}");
  }
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
fn clone(bundle: &Path, expected: &str) -> (std::process::Output, tempfile::TempDir) {
  let parent = tempfile::tempdir().unwrap();
  let output = Command::new("sh")
    .args(["-euc", CLONE])
    .env("SRC", parent.path().join("src"))
    .env("EXPECTED", expected)
    .stdin(Stdio::from(std::fs::File::open(bundle).unwrap()))
    .output()
    .unwrap();
  (output, parent)
}

#[test]
fn the_clone_stage_checks_out_head_with_origin_main_and_refuses_another_sha() {
  let (dir, head) = repo();
  let (file, bundled) = bundle(dir.path()).unwrap();
  assert_eq!(bundled, head);
  let (output, parent) = clone(&file, &head);
  assert!(output.status.success(), "{output:?}");
  let src = parent.path().join("src");
  assert_eq!(git(&src, &["rev-parse", "HEAD"]), head);
  assert_eq!(git(&src, &["rev-parse", "refs/remotes/origin/main"]), head);
  let wrong = "0".repeat(40);
  let (output, _) = clone(&file, &wrong);
  assert!(
    !output.status.success(),
    "a bundle whose HEAD is not the expected SHA was accepted"
  );
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains(&format!("the bundled HEAD is {head}, not {wrong}")),
    "{stderr}"
  );
}

fn args(command: &Command) -> Vec<String> {
  command
    .get_args()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect()
}

#[test]
fn the_toolchain_channel_is_read_from_rust_toolchain_toml() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join("rust-toolchain.toml"),
    "[toolchain]\nchannel = \"1.2.3\"\n",
  )
  .unwrap();
  assert_eq!(channel(dir.path()).unwrap(), "1.2.3");
  std::fs::write(dir.path().join("rust-toolchain.toml"), "[toolchain]\n").unwrap();
  assert!(
    channel(dir.path())
      .unwrap_err()
      .contains("no toolchain.channel")
  );
}

#[test]
fn the_build_passes_the_channel_that_the_dockerfile_installs() {
  let channel = channel(Path::new(ROOT)).unwrap();
  let build = args(&build_command(&channel, Path::new("/tmp/id")));
  assert!(build.contains(&format!("TOOLCHAIN={channel}")), "{build:?}");
  assert!(
    build.contains(&format!("toolu-deploy-gate:{channel}")),
    "{build:?}"
  );
  assert!(
    build
      .windows(2)
      .any(|pair| pair == ["--iidfile", "/tmp/id"]),
    "{build:?}"
  );
  for line in [
    "FROM rust:${TOOLCHAIN}-trixie",
    "rustup toolchain install \"$TOOLCHAIN\"",
  ] {
    assert!(DOCKERFILE.contains(line), "the Dockerfile lacks {line}");
  }
}

#[test]
fn the_container_runs_the_built_image_with_ci_runner_cpus() {
  let head = "a".repeat(40);
  let run = args(&container("sha256:feed", &head));
  for pair in [
    ["--cpus", "4"],
    ["-e", "NEXTEST_TEST_THREADS=4"],
    ["-e", &format!("EXPECTED={head}")],
  ] {
    assert!(
      run.windows(2).any(|window| window == pair),
      "{pair:?} in {run:?}"
    );
  }
  assert!(run.contains(&"sha256:feed".to_owned()), "{run:?}");
}

#[test]
fn a_child_that_exits_before_reading_its_input_reports_its_own_status() {
  let input = "x".repeat(1 << 20);
  let status = with_input(
    Command::new("sh").args(["-c", "exit 3"]),
    Stdio::piped(),
    Some(&input),
  )
  .unwrap();
  assert_eq!(status.code(), Some(3));
}
