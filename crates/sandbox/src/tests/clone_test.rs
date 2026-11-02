use std::path::{Path, PathBuf};
use std::process::Command;

use super::{CLONE, clone_argv};
use crate::git_support::git;

fn words(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

/// Run `script` (a shell command) in the clone of `src` at `dst`, from `rel`.
fn in_clone(src: &Path, dst: &Path, rel: &str, script: &str) -> (i32, String) {
  let argv = clone_argv(src, dst, Path::new(rel), &words(&["sh", "-c", script]));
  let output = Command::new(&argv[0])
    .args(&argv[1..])
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .output()
    .unwrap();
  let text =
    String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr);
  (output.status.code().unwrap_or(-1), text)
}

#[test]
fn the_argv_passes_every_value_as_an_argument() {
  let argv = clone_argv(
    Path::new("/src dir"),
    Path::new("/dst"),
    Path::new("a/b"),
    &words(&["cargo", "--version"]),
  );
  assert_eq!(&argv[..4], ["bash", "-euo", "pipefail", "-c"]);
  assert_eq!(argv[4], CLONE);
  assert_eq!(
    &argv[5..],
    [
      "toolu-sandbox-clone",
      "/src dir",
      "/dst",
      "a/b",
      "cargo",
      "--version"
    ]
  );
}

#[test]
fn a_clone_carries_the_changes_whatever_the_git_diff_settings_say() {
  let dir = tempfile::tempdir().unwrap();
  let base = std::fs::canonicalize(dir.path()).unwrap();
  let src = base.join("src");
  std::fs::create_dir_all(&src).unwrap();
  git(&src, &["init", "-q"]).unwrap();
  std::fs::write(src.join("kept.txt"), "one\n").unwrap();
  git(&src, &["add", "-A"]).unwrap();
  git(&src, &["commit", "-q", "-m", "init"]).unwrap();
  // With no prefixes or with mnemonic ones, `git diff` paths are not a/ and b/ and
  // `git apply` would strip the wrong component and drop the change.
  git(&src, &["config", "diff.noprefix", "true"]).unwrap();
  git(&src, &["config", "diff.mnemonicPrefix", "true"]).unwrap();
  std::fs::write(src.join("kept.txt"), "two\n").unwrap();
  let (code, out) = in_clone(&src, &base.join("clone"), "", "cat kept.txt");
  assert_eq!((code, out.as_str()), (0, "two\n"));
}

#[test]
fn a_clone_carries_head_changes_untracked_files_and_remotes_then_resets() {
  let dir = tempfile::tempdir().unwrap();
  let base = std::fs::canonicalize(dir.path()).unwrap();
  let src = base.join("src with space");
  std::fs::create_dir_all(src.join("sub")).unwrap();
  git(&src, &["init", "-q"]).unwrap();
  std::fs::write(src.join("kept.txt"), "one\n").unwrap();
  std::fs::write(src.join("gone.txt"), "gone\n").unwrap();
  std::fs::write(src.join(".gitignore"), "target/\n").unwrap();
  git(&src, &["add", "-A"]).unwrap();
  git(&src, &["commit", "-q", "-m", "init"]).unwrap();
  git(&src, &["remote", "add", "origin", src.to_str().unwrap()]).unwrap();
  git(&src, &["fetch", "-q", "origin"]).unwrap();
  std::fs::write(src.join("kept.txt"), "two\n").unwrap();
  git(&src, &["rm", "-q", "gone.txt"]).unwrap();
  std::fs::write(src.join("sub/new.txt"), "new\n").unwrap();
  std::fs::create_dir_all(src.join("target")).unwrap();
  std::fs::write(src.join("target/host"), "host build\n").unwrap();
  let dst: PathBuf = base.join("clone");

  let (code, out) = in_clone(
    &src,
    &dst,
    "sub",
    "pwd; cat ../kept.txt new.txt; test ! -e ../gone.txt; test ! -e ../target/host; git -C .. branch -r",
  );
  assert_eq!(code, 0, "{out}");
  assert!(
    out.starts_with(&format!("{}\n", dst.join("sub").display())),
    "{out}"
  );
  assert!(out.contains("two\nnew\n"), "{out}");
  assert!(out.contains("origin/"), "{out}");

  let (code, out) = in_clone(
    &src,
    &dst,
    "",
    "mkdir -p target && echo built > target/out && echo dirty >> kept.txt",
  );
  assert_eq!(code, 0, "{out}");
  let (code, out) = in_clone(&src, &dst, "", "cat kept.txt target/out; exit 3");
  assert_eq!(code, 3, "{out}");
  assert_eq!(out, "two\nbuilt\n");
}

#[test]
#[cfg(target_os = "linux")]
fn clone_lock_does_not_escape_into_background_children() {
  let dir = tempfile::tempdir().unwrap();
  let src = dir.path().join("src");
  std::fs::create_dir_all(&src).unwrap();
  git(&src, &["init", "-q"]).unwrap();
  std::fs::write(src.join("input"), "input\n").unwrap();
  git(&src, &["add", "input"]).unwrap();
  git(&src, &["commit", "-q", "-m", "input"]).unwrap();
  let dst = dir.path().join("clone");
  let (code, out) = in_clone(
    &src,
    &dst,
    "",
    "sleep 30 >/dev/null 2>&1 & echo $! > daemon.pid; if flock -n ../clone.lock true; then exit 7; fi",
  );
  assert_eq!(code, 0, "{out}");
  let pid = std::fs::read_to_string(dst.join("daemon.pid")).unwrap();
  let lock = dir.path().join("clone.lock");
  let available = Command::new("flock")
    .args(["-n", lock.to_str().unwrap(), "true"])
    .status()
    .unwrap();
  let _ = Command::new("kill").arg(pid.trim()).status();
  assert!(available.success(), "background child retained clone lock");
}

#[test]
#[cfg(target_os = "linux")]
fn clone_command_receives_stdin_while_lock_is_held() {
  use std::io::Write as _;
  let dir = tempfile::tempdir().unwrap();
  let src = dir.path().join("src");
  std::fs::create_dir_all(&src).unwrap();
  git(&src, &["init", "-q"]).unwrap();
  std::fs::write(src.join("input"), "input\n").unwrap();
  git(&src, &["add", "input"]).unwrap();
  git(&src, &["commit", "-q", "-m", "input"]).unwrap();
  let argv = clone_argv(
    &src,
    &dir.path().join("clone"),
    Path::new(""),
    &words(&["cat"]),
  );
  let mut child = Command::new(&argv[0])
    .args(&argv[1..])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  child.stdin.take().unwrap().write_all(b"payload\n").unwrap();
  let output = child.wait_with_output().unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"payload\n");
}

#[test]
#[cfg(target_os = "linux")]
fn termination_stops_the_command_group_before_releasing_the_clone_lock() {
  let dir = tempfile::tempdir().unwrap();
  let src = dir.path().join("src");
  std::fs::create_dir_all(&src).unwrap();
  git(&src, &["init", "-q"]).unwrap();
  std::fs::write(src.join("input"), "input\n").unwrap();
  git(&src, &["add", "input"]).unwrap();
  git(&src, &["commit", "-q", "-m", "input"]).unwrap();
  let dst = dir.path().join("clone");
  let mut parent = spawn_process_group(&src, &dst).unwrap();
  wait_for_command(&dst.join("command.pid")).unwrap();
  let command = std::fs::read_to_string(dst.join("command.pid")).unwrap();
  let descendant = std::fs::read_to_string(dst.join("descendant.pid")).unwrap();
  assert!(
    Command::new("kill")
      .args(["-TERM", &parent.id().to_string()])
      .status()
      .unwrap()
      .success()
  );
  let status = parent.wait().unwrap();
  let alive: Vec<bool> = [&command, &descendant]
    .iter()
    .map(|pid| process_alive(pid).unwrap())
    .collect();
  let available = Command::new("flock")
    .args([
      "-n",
      dir.path().join("clone.lock").to_str().unwrap(),
      "true",
    ])
    .status()
    .unwrap()
    .success();
  for pid in [&command, &descendant] {
    let _ = Command::new("kill").args(["-KILL", pid.trim()]).status();
  }
  assert!(!status.success());
  assert_eq!(
    alive,
    [false, false],
    "command group survived parent termination"
  );
  assert!(available, "clone lock was not released after termination");
}

#[cfg(target_os = "linux")]
fn spawn_process_group(src: &Path, dst: &Path) -> std::io::Result<std::process::Child> {
  let argv = clone_argv(
    src,
    dst,
    Path::new(""),
    &words(&[
      "sh",
      "-c",
      "sleep 30 & echo $! > descendant.pid; echo $$ > command.pid; wait",
    ]),
  );
  let (program, tail) = argv
    .split_first()
    .ok_or_else(|| std::io::Error::other("missing command"))?;
  Command::new(program)
    .args(tail)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
}

#[cfg(target_os = "linux")]
fn wait_for_command(path: &Path) -> Result<(), &'static str> {
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
  while !path.exists() {
    if std::time::Instant::now() >= deadline {
      return Err("command did not start");
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
  }
  Ok(())
}

#[cfg(target_os = "linux")]
fn process_alive(pid: &str) -> std::io::Result<bool> {
  let output = Command::new("ps")
    .args(["-p", pid.trim(), "-o", "stat="])
    .output()?;
  Ok(
    output.status.success()
      && !String::from_utf8_lossy(&output.stdout)
        .trim()
        .starts_with('Z'),
  )
}
