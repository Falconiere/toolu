use std::path::Path;

use serde_json::Value;

use super::super::support::{Repo, git};
use std::collections::BTreeSet;

use super::{default_branch, open_paths, owned, parse, sweep};

#[test]
fn porcelain_rows_parse_with_branch_detached_and_prunable_states() {
  let text = "worktree /r\nHEAD aaa\nbranch refs/heads/main\n\nworktree /r/wt\nHEAD bbb\ndetached\nprunable gitdir file points to non-existent location\n";
  let trees = parse(text);
  assert_eq!(trees.len(), 2);
  assert_eq!(trees[0].branch.as_deref(), Some("main"));
  assert!(!trees[0].prunable);
  assert_eq!(trees[1].branch, None);
  assert!(trees[1].prunable);
}

#[test]
fn only_herdr_babysit_and_hashed_paths_count_as_toolu_owned() {
  assert!(owned("/Volumes/Projects/.herdr/worktrees/toolu/feat-1"));
  assert!(owned("/Volumes/Projects/.machine/pr-babysit-worktrees/x-1"));
  assert!(owned("/repo/../toolu-0123456789abcdef01234567"));
  assert!(!owned("/repo/.claude/worktrees/agent-1"));
  assert!(!owned("/home/me/work/feature"));
}

/// The six worktrees of the removal test: merged, ahead, dirty, session-only
/// dirt, a human's merged one outside toolu's roots, and a deleted one.
fn six_worktrees(repo: &Repo) -> Result<(), String> {
  let herdr = repo.tmp.join(".herdr/worktrees/repo");
  repo.worktree(&herdr.join("merged"), "feat/merged")?;
  repo.worktree(&herdr.join("ahead"), "feat/ahead")?;
  repo.commit(&herdr.join("ahead"), "more")?;
  repo.worktree(&herdr.join("dirty"), "feat/dirty")?;
  std::fs::write(herdr.join("dirty/scratch"), "x").map_err(|e| e.to_string())?;
  repo.worktree(&herdr.join("session"), "feat/session")?;
  std::fs::create_dir_all(herdr.join("session/.claude")).map_err(|e| e.to_string())?;
  std::fs::write(herdr.join("session/.claude/settings.local.json"), "{}")
    .map_err(|e| e.to_string())?;
  repo.worktree(&repo.tmp.join("human"), "feat/human")?;
  repo.worktree(&herdr.join("gone"), "feat/gone")?;
  std::fs::remove_dir_all(herdr.join("gone")).map_err(|e| e.to_string())
}

fn reason(report: &Value, name: &str) -> String {
  report["worktrees"]
    .as_array()
    .unwrap()
    .iter()
    .find(|row| row["path"].as_str().unwrap().ends_with(name))
    .map(|row| row["reason"].as_str().unwrap().to_owned())
    .unwrap()
}

fn action(report: &Value, name: &str) -> String {
  report["worktrees"]
    .as_array()
    .unwrap()
    .iter()
    .find(|row| row["path"].as_str().unwrap().ends_with(name))
    .map(|row| row["action"].as_str().unwrap().to_owned())
    .unwrap()
}

#[test]
fn a_sweep_removes_only_merged_clean_toolu_worktrees_and_prunes_deleted_ones() {
  let repo = Repo::new().unwrap();
  assert_eq!(default_branch(&repo.env, &repo.root), "main");
  six_worktrees(&repo).unwrap();
  let herdr = repo.tmp.join(".herdr/worktrees/repo");

  let report = sweep(&repo.request(false), &repo.root, &BTreeSet::new());
  assert_eq!(action(&report, "merged"), "would-remove");
  assert_eq!(action(&report, "session"), "would-remove");
  assert_eq!(action(&report, "ahead"), "kept");
  assert_eq!(action(&report, "dirty"), "kept");
  assert_eq!(action(&report, "human"), "kept");
  assert_eq!(action(&report, "gone"), "pruned");
  assert!(herdr.join("merged").exists(), "a report removes nothing");

  let applied = sweep(&repo.request(true), &repo.root, &BTreeSet::new());
  assert_eq!(action(&applied, "merged"), "removed");
  let left: Vec<bool> = ["merged", "session", "ahead", "dirty"]
    .iter()
    .map(|name| herdr.join(name).exists())
    .collect();
  assert_eq!(left, [false, false, true, true]);
  assert!(repo.tmp.join("human").exists());
  assert_eq!(applied["pruned"], 1);
  let listed = git(&repo.env, &repo.root, &["worktree", "list"]).unwrap();
  assert!(!listed.contains("gone"), "{listed}");

  let again = sweep(&repo.request(true), &repo.root, &BTreeSet::new());
  let removed = again["worktrees"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|row| row["action"] == "removed")
    .count();
  assert_eq!(removed, 0, "a second sweep finds nothing to remove");
}

#[test]
fn a_squash_merged_worktree_is_kept_and_named_for_the_human() {
  let repo = Repo::new().unwrap();
  let path = repo.tmp.join(".herdr/worktrees/repo/squashed");
  repo.worktree(&path, "feat/squashed").unwrap();
  repo.commit(&path, "work").unwrap();
  git(
    &repo.env,
    &path,
    &["push", "-q", "-u", "origin", "feat/squashed"],
  )
  .unwrap();
  git(
    &repo.env,
    &repo.root,
    &["push", "-q", "origin", "--delete", "feat/squashed"],
  )
  .unwrap();
  git(&repo.env, &repo.root, &["fetch", "-q", "--prune", "origin"]).unwrap();
  let report = sweep(&repo.request(true), &repo.root, &BTreeSet::new());
  assert_eq!(action(&report, "squashed"), "kept");
  assert_eq!(
    report["worktrees"][0]["reason"],
    "not merged into refs/remotes/origin/main"
  );
  assert!(path.exists());
}

#[test]
fn a_worktree_a_lease_or_busy_record_holds_stays_even_when_merged_and_clean() {
  let repo = Repo::new().unwrap();
  let herdr = repo.tmp.join(".herdr/worktrees/repo");
  repo.worktree(&herdr.join("fresh"), "feat/fresh").unwrap();
  let held = BTreeSet::from([herdr.join("fresh").display().to_string()]);
  let report = sweep(&repo.request(true), &repo.root, &held);
  assert_eq!(action(&report, "fresh"), "kept");
  assert_eq!(reason(&report, "fresh"), "a worker holds it");
  assert!(herdr.join("fresh").exists());
}

#[test]
fn open_workspaces_are_read_from_a_wrapped_or_bare_herdr_reply() {
  let rows = serde_json::json!([
    {"path": "/w/open", "open_workspace_id": "w1"},
    {"path": "/w/closed", "open_workspace_id": ""},
    {"path": "/w/none"}
  ]);
  let wrapped = serde_json::json!({"result": {"worktrees": rows}});
  let bare = serde_json::json!({"worktrees": rows});
  let expected = BTreeSet::from(["/w/open".to_owned()]);
  assert_eq!(open_paths(&wrapped), expected);
  assert_eq!(open_paths(&bare), expected);
  assert!(open_paths(&serde_json::json!({})).is_empty());
}

#[test]
fn a_repository_without_an_origin_default_branch_is_skipped() {
  let repo = Repo::new().unwrap();
  let lone = repo.tmp.join("lone");
  git(&repo.env, &repo.tmp, &["init", "-q", "-b", "main", "lone"]).unwrap();
  let report = sweep(&repo.request(false), Path::new(&lone), &BTreeSet::new());
  assert_eq!(report["skipped"], "no refs/remotes/origin/main");
}
