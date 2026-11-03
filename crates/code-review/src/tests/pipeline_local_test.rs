use std::path::Path;
use std::process::Command;

use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;

use super::fixture_test::{clean_case, env, invoke, options, review_with_finding, saved_state};
use crate::contract::Verdict;

pub(super) fn git_output(root: &Path, args: &[&str]) -> Vec<u8> {
  let output = Command::new("git")
    .current_dir(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  output.stdout
}

#[test]
fn local_only_review_includes_pending_content_without_changing_git_state() {
  let (repo, state, fixture) = clean_case();
  let root = repo.path();
  git_output(root, &["reset", "--hard", "main"]);
  std::fs::write(root.join("src.txt"), "staged content\n").unwrap();
  git_output(root, &["add", "src.txt"]);
  std::fs::write(root.join("src.txt"), "unstaged content\n").unwrap();
  std::fs::write(root.join("new.txt"), "untracked content\n").unwrap();
  std::fs::write(root.join("ignored.txt"), "ignored content\n").unwrap();
  std::fs::write(root.join(".git/info/exclude"), "ignored.txt\n").unwrap();
  let head = git_output(root, &["rev-parse", "HEAD"]);
  let index = std::fs::read(root.join(".git/index")).unwrap();
  let env = env(root, state.path(), &fixture);
  let roots = Roots::new(env.clone(), Some(Host::Codex));
  let shaped = super::super::review_shape(&env, &roots, root, Some("main")).unwrap();
  let patches = shaped
    .files
    .iter()
    .map(|file| file.patch.as_str())
    .collect::<String>();
  assert!(patches.contains("unstaged content"), "{patches}");
  assert!(patches.contains("untracked content"), "{patches}");
  assert!(
    !patches.lines().any(|line| line == "+staged content"),
    "{patches}"
  );
  assert!(!patches.contains("ignored content"), "{patches}");
  let report = invoke(&env, &options(&fixture));
  assert!(report.coverage.reviewed.contains(&"src.txt".to_owned()));
  assert!(report.coverage.reviewed.contains(&"new.txt".to_owned()));
  assert_ne!(report.verdict(), Verdict::Error, "{report:?}");
  let gate = state.path().join("feature.json");
  if gate.exists() {
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(gate).unwrap()).unwrap();
    assert!(saved["findings_count"].as_u64().unwrap() > 0);
  }
  assert_eq!(git_output(root, &["rev-parse", "HEAD"]), head);
  assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
}

#[test]
fn local_snapshot_drives_real_structural_scanner() {
  let (repo, state, fixture) = clean_case();
  let root = repo.path();
  std::fs::write(root.join("pending.rs"), "fn pending() { todo!() }\n").unwrap();
  let env = env(root, state.path(), &fixture);
  let roots = Roots::new(env.clone(), Some(Host::Codex));
  let shaped = super::super::review_shape(&env, &roots, root, Some("main")).unwrap();
  std::fs::write(root.join("pending.rs"), "fn pending() {}\n").unwrap();
  let scanned = crate::prepass::run(&env, root, &shaped);
  assert!(scanned.notices.is_empty(), "{:?}", scanned.notices);
  assert!(
    scanned
      .findings
      .iter()
      .any(|finding| { finding.path == "pending.rs" && finding.category == "review-todo-macro" }),
    "{:?}",
    scanned.findings
  );
}

#[test]
fn committing_local_snapshot_during_review_does_not_approve_push() {
  use crate::budget::Effort;
  use crate::contract::{Coverage, Report, Usage};
  use crate::prepass::PrepassResult;
  use crate::validate::Recap;
  use toolu_runtime::cli::Ctx;

  let (repo, state, fixture) = clean_case();
  let root = repo.path();
  std::fs::write(root.join("src.txt"), "pending\n").unwrap();
  let env = env(root, state.path(), &fixture);
  let roots = Roots::new(env.clone(), Some(Host::Codex));
  let shaped = super::super::review_shape(&env, &roots, root, Some("main")).unwrap();
  git_output(root, &["add", "src.txt"]);
  git_output(root, &["commit", "-qm", "commit during review"]);
  let prepass = PrepassResult {
    findings: Vec::new(),
    redactions: Vec::new(),
    notices: Vec::new(),
    overflow: false,
  };
  let ctx = Ctx {
    host: Some(Host::Codex),
    ..Ctx::default()
  };
  let memory = root.join(".codex/tmp/code-review/feature.json");
  let run = super::super::state::RunState {
    env: &env,
    ctx: &ctx,
    roots: &roots,
    root,
    shaped: &shaped,
    prepass: &prepass,
    scoped: false,
    memory_path: &memory,
  };
  let mut report = Report::reviewed(
    Effort::Medium,
    Vec::new(),
    Coverage::default(),
    Usage::default(),
  );
  super::super::state::persist(&run, &mut report, &Recap::default()).unwrap();
  let saved = saved_state(state.path());
  assert!(saved["findings_count"].as_u64().unwrap() > 0);
  let committed_sha = toolu_state::diff_sha::diff_sha(&env, root, "main").unwrap();
  assert_eq!(saved["diff_sha"], committed_sha);
}

#[test]
fn local_blocking_finding_replaces_prior_passing_push_state() {
  let (repo, state, clean) = clean_case();
  let root = repo.path();
  let clean_env = env(root, state.path(), &clean);
  assert_eq!(
    invoke(&clean_env, &options(&clean)).verdict(),
    Verdict::Approved
  );
  let before = saved_state(state.path());
  assert_eq!(before["findings_count"], 0);

  std::fs::write(root.join("pending.txt"), "local change\n").unwrap();
  let env = review_with_finding(root, state.path());
  let saved = saved_state(state.path());
  assert!(saved["findings_count"].as_u64().unwrap() > 0);
  let committed_sha = toolu_state::diff_sha::diff_sha(&env, root, "main").unwrap();
  assert_eq!(saved["diff_sha"], committed_sha);
}

#[test]
fn local_review_combines_committed_and_staged_changes() {
  let (repo, state, fixture) = clean_case();
  let root = repo.path();
  std::fs::write(root.join("staged.txt"), "staged content\n").unwrap();
  git_output(root, &["add", "staged.txt"]);
  let env = env(root, state.path(), &fixture);
  let roots = Roots::new(env.clone(), Some(Host::Codex));
  let shaped = super::super::review_shape(&env, &roots, root, Some("main")).unwrap();
  let patches = shaped
    .files
    .iter()
    .map(|file| file.patch.as_str())
    .collect::<String>();
  assert!(patches.contains("staged content"), "{patches}");
  assert!(patches.contains("pub fn value"), "{patches}");
}

#[test]
fn snapshot_keeps_force_added_ignored_files_and_renames_with_split_index() {
  let (repo, state, fixture) = clean_case();
  let root = repo.path();
  git_output(root, &["reset", "--hard", "main"]);
  std::fs::write(root.join("remove.txt"), "delete me\n").unwrap();
  git_output(root, &["add", "remove.txt"]);
  git_output(root, &["commit", "-qm", "base additions"]);
  git_output(root, &["branch", "-f", "main", "HEAD"]);
  git_output(root, &["update-index", "--split-index"]);
  std::fs::rename(root.join("src.txt"), root.join("moved.txt")).unwrap();
  std::fs::remove_file(root.join("remove.txt")).unwrap();
  std::fs::write(root.join(".git/info/exclude"), "ignored.txt\n").unwrap();
  std::fs::write(root.join("ignored.txt"), "force added\n").unwrap();
  git_output(root, &["add", "-f", "ignored.txt"]);
  let index = std::fs::read(root.join(".git/index")).unwrap();
  let status = git_output(
    root,
    &["--no-optional-locks", "status", "--porcelain=v1", "-z"],
  );
  let env = env(root, state.path(), &fixture);
  let roots = Roots::new(env.clone(), Some(Host::Codex));
  let shaped = super::super::review_shape(&env, &roots, root, Some("main")).unwrap();
  assert!(
    shaped
      .files
      .iter()
      .any(|file| file.path == "ignored.txt" && file.patch.contains("force added"))
  );
  assert!(
    shaped
      .renames
      .iter()
      .any(|item| item.old_path == "src.txt" && item.path == "moved.txt")
  );
  assert!(
    shaped
      .files
      .iter()
      .any(|file| file.path == "remove.txt" && file.kind == crate::shape::FileKind::Deleted)
  );
  assert_eq!(
    git_output(
      root,
      &["--no-optional-locks", "status", "--porcelain=v1", "-z"]
    ),
    status
  );
  assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
}

#[test]
fn unresolved_merge_conflict_is_rejected_without_changing_index() {
  let (repo, state, fixture) = clean_case();
  let root = repo.path();
  git_output(root, &["switch", "main"]);
  std::fs::write(root.join("src.txt"), "main change\n").unwrap();
  git_output(root, &["add", "src.txt"]);
  git_output(root, &["commit", "-qm", "main change"]);
  git_output(root, &["switch", "feature"]);
  let merge = Command::new("git")
    .current_dir(root)
    .args(["merge", "main"])
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .output()
    .unwrap();
  assert!(!merge.status.success());
  assert_ne!(git_output(root, &["ls-files", "-u"]), Vec::<u8>::new());
  let index = std::fs::read(root.join(".git/index")).unwrap();
  let env = env(root, state.path(), &fixture);
  let roots = Roots::new(env.clone(), Some(Host::Codex));
  assert!(super::super::review_shape(&env, &roots, root, Some("main")).is_err());
  assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
}
