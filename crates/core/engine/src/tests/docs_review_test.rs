use super::*;
use serde_json::json;
use std::process::Command;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

fn git(repo: &Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("git");
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
}

pub(super) fn fixture() -> (tempfile::TempDir, Roots) {
  let dir = tempfile::tempdir().expect("tempdir");
  let repo = dir.path();
  git(repo, &["init", "-b", "main"]);
  git(repo, &["config", "user.email", "test@example.com"]);
  git(repo, &["config", "user.name", "Test"]);
  std::fs::write(repo.join("README.md"), "initial\n").expect("readme");
  git(repo, &["add", "."]);
  git(repo, &["commit", "-m", "initial"]);
  git(repo, &["checkout", "-b", "feature"]);
  std::fs::write(repo.join("code.rs"), "change\n").expect("code");
  std::fs::write(repo.join("README.md"), "updated\n").expect("readme");
  git(repo, &["add", "."]);
  git(repo, &["commit", "-m", "change"]);
  let env = Env::from_pairs([
    ("HOME", repo.to_str().expect("path")),
    ("DOCS_SYNC_BASE", "main"),
  ]);
  (dir, Roots::new(env, Some(Host::Claude)))
}

pub(super) fn report(roots: &Roots, repo: &Path) -> Value {
  let mut report = inspect(roots, repo, None).expect("inspect");
  report["route"] =
    json!({"source":"jev","candidate_id":"review","host":"codex","model":"model","effort":"high"});
  report["worker"] = json!({"id":"worker","host":"codex","model":"model","effort":"high"});
  report["coverage"] = json!([
    {"path":"README.md","disposition":"updated","note":"Updated the guide","docs":["README.md"]},
    {"path":"code.rs","disposition":"accurate","note":"Guide describes behavior","docs":["README.md"]}
  ]);
  report["checks"] = json!(["Checked guide against implementation"]);
  report["findings"] = json!([]);
  report
}

#[test]
fn complete_review_is_current_and_rejects_dirty_or_stale_inputs() {
  let (dir, roots) = fixture();
  let repo = dir.path();
  assert!(assess(&roots, repo, "feature").is_err());
  let state = report(&roots, repo);
  write(&roots, repo, None, &state).expect("write review");
  assess(&roots, repo, "feature").expect("review valid");
  std::fs::write(repo.join("code.rs"), "dirty\n").expect("dirty");
  assert!(write(&roots, repo, None, &state).is_err());
  git(repo, &["add", "code.rs"]);
  git(repo, &["commit", "-m", "new diff"]);
  assert!(assess(&roots, repo, "feature").is_err());
}

#[test]
fn detached_review_requires_a_known_local_branch() {
  let (dir, roots) = fixture();
  let repo = dir.path();
  git(repo, &["checkout", "--detach"]);
  assert!(inspect(&roots, repo, None).is_err());
  assert!(inspect(&roots, repo, Some("unknown")).is_err());
  assert!(inspect(&roots, repo, Some("feature")).is_ok());
  git(
    repo,
    &["update-ref", "refs/remotes/origin/remote-only", "HEAD"],
  );
  assert!(inspect(&roots, repo, Some("remote-only")).is_ok());
}
