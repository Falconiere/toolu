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
