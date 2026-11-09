use std::path::Path;
use std::process::Command;

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;
use toolu_state::diff_sha::diff_sha;

use super::{LocalGate, apply, policy, short};

fn env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap_or_default())])
}

/// `git -C dir args`, trimmed; the test fails on a non-zero exit.
fn git(dir: &Path, args: &[&str]) -> String {
  let out = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(["-c", "user.name=t", "-c", "user.email=t@t"])
    .args(args)
    .output()
    .expect("git");
  assert!(
    out.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// `main` with one commit and `feat` adding a file; the head of `feat`.
fn repo(dir: &Path) -> String {
  git(dir, &["init", "-q", "-b", "main"]);
  git(dir, &["commit", "-q", "--allow-empty", "-m", "base"]);
  git(dir, &["checkout", "-q", "-b", "feat"]);
  std::fs::write(dir.join("a.rs"), "hi\n").expect("file");
  git(dir, &["add", "."]);
  git(dir, &["commit", "-q", "-m", "feat"]);
  git(dir, &["rev-parse", "HEAD"])
}

/// A one-step ledger for `feat` under Claude's state dir, green at `sha`.
fn ledger(dir: &Path, sha: &str) {
  let file = dir.join(".claude/tmp/plan-ledger/feat.json");
  std::fs::create_dir_all(file.parent().expect("dir")).expect("ledger dir");
  let doc = json!({
    "version": 1, "branch": "feat", "base_branch": "main", "next": null,
    "summary": {"total": 1, "fresh_green": 1},
    "steps": [{"id": "gate", "title": "full gate", "status": "green", "diff_sha": sha}],
  });
  std::fs::write(file, doc.to_string()).expect("ledger");
}

fn assessed(verdict: &str, head: &str) -> Map<String, Value> {
  let mut result = Map::new();
  result.insert("verdict".to_owned(), json!(verdict));
  result.insert("head".to_owned(), json!(head));
  result.insert("mergeable".to_owned(), json!("MERGEABLE"));
  result.insert(
    "reasons".to_owned(),
    json!(["pending: (no checks reported yet)"]),
  );
  result
}

fn verdict(result: &Map<String, Value>) -> (String, String) {
  let text = |key: &str| result[key].as_str().unwrap_or("").to_owned();
  let reason = result["reasons"][0].as_str().unwrap_or("").to_owned();
  (text("verdict"), reason)
}

#[test]
fn a_fresh_green_ledger_at_the_pull_request_head_merges_and_anything_less_waits_on_the_worker() {
  let tmp = tempfile::tempdir().expect("temp");
  let head = repo(tmp.path());
  let local = LocalGate {
    worktree: Some(tmp.path().to_path_buf()),
    host: Some("claude".to_owned()),
  };
  let mut result = assessed("wait", &head);
  apply(&mut result, &local, &env());
  let (state, reason) = verdict(&result);
  assert_eq!(state, "wait");
  assert!(reason.starts_with("local gate: no plan ledger"), "{reason}");
  assert_eq!(result["local_gate"], json!(false));
  assert_eq!(result["local_wait"], json!(true), "the worker owes it");
  // A green ledger goes stale after the engine's own rebase: the worker owes
  // it, so the queue step prompts instead of blocking the queue.
  ledger(tmp.path(), "0000000");
  let mut result = assessed("wait", &head);
  apply(&mut result, &local, &env());
  let (state, reason) = verdict(&result);
  assert_eq!(state, "wait");
  assert_eq!(reason, "local gate: steps not fresh-green: gate: stale");
  let cur = diff_sha(&env(), tmp.path(), "main").expect("diff sha");
  ledger(tmp.path(), &cur);
  let mut result = assessed("wait", &head);
  apply(&mut result, &local, &env());
  let (state, reason) = verdict(&result);
  assert_eq!(state, "merge");
  assert_eq!(
    reason,
    format!(
      "local gate on {}: all plan-ledger steps fresh-green",
      short(&head)
    )
  );
  assert_eq!(result["local_gate"], json!(true));
  let mut moved = assessed("merge", "deadbeef0");
  apply(&mut moved, &local, &env());
  let (state, reason) = verdict(&moved);
  assert_eq!(state, "wait");
  assert_eq!(moved["local_wait"], json!(true));
  assert_eq!(
    reason,
    format!(
      "local gate: worktree at {}, pull request head deadbee",
      short(&head)
    )
  );
}

/// toolu inside Grok Build acts for Claude Code, so a Grok worker's ledger is
/// under `.claude/`; an unknown host is still no host.
#[test]
fn a_grok_worker_merges_on_its_ledger_under_claude_code() {
  let tmp = tempfile::tempdir().expect("temp");
  let head = repo(tmp.path());
  let cur = diff_sha(&env(), tmp.path(), "main").expect("diff sha");
  ledger(tmp.path(), &cur);
  let grok = LocalGate {
    worktree: Some(tmp.path().to_path_buf()),
    host: Some("grok".to_owned()),
  };
  let mut result = assessed("wait", &head);
  apply(&mut result, &grok, &env());
  assert_eq!(verdict(&result).0, "merge");
  let unknown = LocalGate {
    host: Some("gemini".to_owned()),
    ..grok
  };
  let mut result = assessed("wait", &head);
  apply(&mut result, &unknown, &env());
  assert_eq!(
    verdict(&result),
    ("wait".to_owned(), "local gate: no host recorded".to_owned())
  );
}

#[test]
fn the_local_gate_leaves_fix_rebase_and_unknown_mergeability_alone() {
  let local = LocalGate {
    worktree: None,
    host: None,
  };
  for state in ["fix", "rebase", "merged", "closed"] {
    let mut result = assessed(state, "abc");
    apply(&mut result, &local, &env());
    assert_eq!(verdict(&result).0, state);
    assert!(result.get("local_gate").is_none());
  }
  let mut unknown = assessed("wait", "abc");
  unknown.insert("mergeable".to_owned(), json!("UNKNOWN"));
  apply(&mut unknown, &local, &env());
  assert_eq!(
    verdict(&unknown).1,
    "pending: (no checks reported yet)",
    "GitHub still computing"
  );
  let mut bare = assessed("wait", "abc");
  apply(&mut bare, &local, &env());
  assert_eq!(
    verdict(&bare),
    (
      "wait".to_owned(),
      "local gate: no worktree recorded".to_owned()
    )
  );
  assert_eq!(bare["local_wait"], json!(false), "the engine owes it");
  let mut no_host = assessed("wait", "abc");
  let worktree = LocalGate {
    worktree: Some("/wt".into()),
    host: None,
  };
  apply(&mut no_host, &worktree, &env());
  assert_eq!(verdict(&no_host).1, "local gate: no host recorded");
}

#[test]
fn the_policy_reads_the_start_flag_and_the_launch_record() {
  let tmp = tempfile::tempdir().expect("temp");
  let fallback = LocalGate {
    worktree: Some("/engine/wt".into()),
    host: Some("codex".to_owned()),
  };
  let record = json!({"worktree": "/record/wt", "kind": "muse"});
  assert!(
    policy(tmp.path(), &record, &fallback).is_none(),
    "babysit on"
  );
  let settings = crate::epics::LaunchSettings {
    no_babysit: true,
    ..crate::epics::LaunchSettings::default()
  };
  crate::epics::write_launch_settings(tmp.path(), settings).expect("settings");
  let from_record = policy(tmp.path(), &record, &fallback).expect("policy");
  assert_eq!(
    from_record,
    LocalGate {
      worktree: Some("/record/wt".into()),
      host: Some("muse".to_owned()),
    }
  );
  let from_fallback = policy(tmp.path(), &json!({"worktree": ""}), &fallback).expect("policy");
  assert_eq!(from_fallback, fallback);
}
