use std::path::Path;
use std::process::Command;

use toolu_runtime::env::Env;

use super::diff_sha;

const EMPTY_BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

fn env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap())])
}

fn sh(dir: &Path, script: &str) -> String {
  let out = Command::new("sh")
    .args(["-c", script])
    .current_dir(dir)
    .output()
    .unwrap();
  assert!(
    out.status.success(),
    "{script}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// `main` with one commit and `feat` adding a non-UTF-8 file and a 1.5 MiB one.
fn branch_repo(dir: &Path) {
  sh(
    dir,
    "git init -q -b main && git -c user.name=t -c user.email=t@t commit -q --allow-empty -m base \
     && git checkout -q -b feat && printf 'a\\377\\376b\\n' > bytes.txt \
     && head -c 1572864 /dev/zero | tr '\\0' 'x' | fold -w 100 > big.txt \
     && git add . && git -c user.name=t -c user.email=t@t commit -q -m feat",
  );
}

#[test]
fn a_branch_diff_hashes_as_gits_own_pipeline_does() {
  let dir = tempfile::tempdir().unwrap();
  branch_repo(dir.path());
  let piped = sh(
    dir.path(),
    "git diff --no-color main...HEAD | git hash-object --stdin",
  );
  let ours = diff_sha(&env(), dir.path(), "main").unwrap();
  assert_eq!(ours, piped);
  assert_ne!(ours, EMPTY_BLOB);
  assert_eq!(
    diff_sha(&env(), dir.path(), "feat").as_deref(),
    Some(EMPTY_BLOB)
  );
}

#[test]
fn bad_refs_dash_refs_and_non_repositories_give_no_hash() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(
    diff_sha(&env(), dir.path(), "main"),
    None,
    "not a repository"
  );
  branch_repo(dir.path());
  assert_eq!(diff_sha(&env(), dir.path(), "nope"), None);
  let planted = dir.path().join("planted");
  assert_eq!(
    diff_sha(
      &env(),
      dir.path(),
      &format!("--output={}", planted.display())
    ),
    None
  );
  assert!(!planted.exists());
  assert_eq!(
    diff_sha(&Env::from_pairs([("PATH", "")]), dir.path(), "main"),
    None,
    "no git"
  );
}
