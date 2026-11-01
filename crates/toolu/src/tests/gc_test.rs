use serde_json::json;
use toolu_runtime::env::Env;

use super::{command, lines, state_dirs};

#[test]
fn gc_takes_apply_and_repeated_repos() {
  let matches = command()
    .try_get_matches_from(["gc", "--apply", "--repo", "/a", "--repo", "/b"])
    .unwrap();
  assert!(matches.get_flag("apply"));
  let repos: Vec<&String> = matches.get_many::<String>("repo").unwrap().collect();
  assert_eq!(repos, ["/a", "/b"]);
  assert!(
    !command()
      .try_get_matches_from(["gc"])
      .unwrap()
      .get_flag("apply")
  );
}

#[test]
fn state_dirs_come_from_the_registry_its_siblings_and_the_state_home() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  for sub in [
    "resources",
    "epics/e1/issues",
    "epics/e2/issues",
    "epics/notes",
    "home/.claude/epics/e3/issues",
  ] {
    std::fs::create_dir_all(root.join(sub)).unwrap();
  }
  std::fs::write(
    root.join("resources/registry.json"),
    json!({"version": 1, "epics": [{"key": "e1", "state_dir": root.join("epics/e1")}]}).to_string(),
  )
  .unwrap();
  let env = Env::from_pairs([("HOME", root.join("home").display().to_string())]);
  let found = state_dirs(&env, &root.join("resources"));
  assert_eq!(
    found,
    [
      root.join("epics/e1"),
      root.join("epics/e2"),
      root.join("home/.claude/epics/e3")
    ]
  );
  let scoped = Env::from_pairs([("EPIC_STATE_HOME", root.join("epics").display().to_string())]);
  assert_eq!(
    state_dirs(&scoped, &root.join("none")),
    [root.join("epics/e1"), root.join("epics/e2")]
  );
}

#[test]
fn the_text_report_names_each_decision_and_hides_foreign_worktrees() {
  let report = json!({
    "apply": false,
    "repos": [{
      "root": "/r",
      "worktrees": [
        {"path": "/r/.herdr/worktrees/r/a", "action": "would-remove", "reason": "merged and clean"},
        {"path": "/r/.claude/worktrees/x", "action": "kept", "reason": "not made by toolu"}
      ],
      "branches": {"removed": ["feat/a"], "kept": [{"name": "busy", "reason": "checked out"}]},
      "wip_refs": {"removed": ["refs/epic-wip/a"], "kept": []}
    }, {"root": "/lone", "skipped": "no refs/remotes/origin/main"}],
    "sandbox": {"action": "would-stop", "reason": "no lease is held", "clones": {"removed": [{"slug": "x"}], "kept": [], "error": "no VM"}},
    "colima": {"toolu-gate": "stopped"},
    "docker": {"dangling": 2},
    "engine": {"job_logs": 5, "job_logs_removed": 0, "stale_records": [{"key": "k", "stage": "running", "fix": "toolu epic finish --abandon /s k"}]}
  });
  let text = lines(&report);
  assert_eq!(
    text[0],
    "gc: would: worktrees=1 branches=1 wip_refs=1 job_logs=0 sandbox=would-stop colima=stopped"
  );
  assert!(
    text.contains(&"  worktree would-remove /r/.herdr/worktrees/r/a: merged and clean".to_owned())
  );
  assert!(!text.iter().any(|line| line.contains(".claude/worktrees")));
  assert!(text.contains(&"  branches: feat/a".to_owned()));
  assert!(text.contains(&"  branch kept busy: checked out".to_owned()));
  assert!(text.contains(&"  wip refs: 1 to remove, 0 kept".to_owned()));
  assert!(text.contains(&"  skipped: no refs/remotes/origin/main".to_owned()));
  assert!(text.contains(&"sandbox: would-stop no lease is held".to_owned()));
  assert!(text.contains(&"  clones: 1 orphan, 0 kept".to_owned()));
  assert!(text.contains(&"  clones error: no VM".to_owned()));
  assert!(text.contains(&"docker: 2 dangling images".to_owned()));
  assert!(text.contains(&"stale record k (running): toolu epic finish --abandon /s k".to_owned()));
}
