use super::*;

#[test]
fn registration_is_idempotent_and_rejects_invalid_manifest() {
  let dir = tempfile::tempdir().expect("tempdir");
  let config = dir.path().to_str().expect("path");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", config)]);
  let roots = Roots::new(env.clone(), Some(Host::Claude));
  assert!(!active(&roots));
  assert_eq!(register(&env, Host::Claude).stdout, None);
  assert!(active(&roots));
  let path = event_dir(&roots, RegistryEvent::ToolPre)
    .join(file_name(SPEC, NAME, ModuleKind::Manifest).expect("file name"));
  let before = std::fs::metadata(&path)
    .expect("metadata")
    .modified()
    .expect("mtime");
  register(&env, Host::Claude);
  assert_eq!(
    before,
    std::fs::metadata(&path)
      .expect("metadata")
      .modified()
      .expect("mtime")
  );
  std::fs::write(path, "{}\n").expect("corrupt manifest");
  assert!(!active(&roots));
}

#[test]
fn manifest_is_active_for_unknown_or_installed_but_not_absent_plugins() {
  let dir = tempfile::tempdir().expect("tempdir");
  let config = dir.path().to_str().expect("path");
  for host in [Host::Claude, Host::Codex] {
    let env = Env::from_pairs([("TOOLU_CONFIG_DIR", config)]);
    let roots = Roots::new(env.clone(), Some(host));
    register(&env, host);
    assert!(active(&roots), "unknown {host:?}");
    let (path, absent, installed) = if host == Host::Claude {
      (
        dir.path().join("plugins/installed_plugins.json"),
        r#"{"version":2,"plugins":{}}"#,
        r#"{"version":2,"plugins":{"documentation-gate@toolu":[]}}"#,
      )
    } else {
      (
        dir.path().join("toolu/codex-plugins.json"),
        r#"{"version":1,"status":"ready","plugins":[]}"#,
        r#"{"version":1,"status":"ready","plugins":["documentation-gate@toolu"]}"#,
      )
    };
    std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
    std::fs::write(&path, absent).expect("record");
    assert!(!active(&roots), "absent {host:?}");
    std::fs::write(&path, installed).expect("record");
    assert!(active(&roots), "installed {host:?}");
  }
}

#[test]
fn push_to_another_repo_uses_its_project_config() {
  use crate::ledger::context::test_repo::Repo;
  let source = Repo::new().unwrap();
  let target = Repo::new().unwrap();
  source.sh("mkdir -p .claude && echo '{\"version\":1,\"docsSync\":{\"mode\":\"off\"}}' > .claude/toolu.config.json").unwrap();
  target.sh("mkdir -p .claude && echo '{\"version\":1,\"docsSync\":{\"mode\":\"block\"}}' > .claude/toolu.config.json").unwrap();
  let env = source
    .env()
    .with("TOOLU_PROJECT_DIR", &source.root.to_string_lossy());
  let command = format!("git -C '{}' push origin feat/x", target.root.display());
  let raw = serde_json::json!({"tool_input":{"command":command}})
    .as_object()
    .unwrap()
    .clone();
  let event = push_event(&command);
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: &source.root,
    project_root: &source.root,
    cwd: Some(&source.root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  assert!(matches!(RULE.run(&event, &ctx), Decision::Deny { .. }));
  target
    .sh("echo '{\"version\":1,\"docsSync\":{\"mode\":\"off\"}}' > .claude/toolu.config.json")
    .unwrap();
  assert!(matches!(RULE.run(&event, &ctx), Decision::Allow));
}

fn push_event(command: &str) -> NormalizedEvent {
  use toolu_protocol::{
    normalized::{Session, Tool},
    text::Text,
  };
  let text = |s: &str| Text::new(s).unwrap();
  let session = Session {
    session_id: text("id"),
    cwd: text("cwd"),
    project_root: text("root"),
    worktree: text("tree"),
  };
  let tool = Tool {
    call_id: text("id"),
    name: text("Bash"),
    input: serde_json::Map::new(),
  };
  NormalizedEvent::ShellPre {
    session,
    tool,
    command: text(command),
  }
}

#[test]
fn rule_applies_to_shell_events_only() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("HOME", dir.path().to_str().unwrap())]);
  let raw = serde_json::Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: dir.path(),
    project_root: dir.path(),
    cwd: Some(dir.path()),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let shell = push_event("git push");
  let other = NormalizedEvent::SessionStart(shell.session().clone());
  assert!(RULE.applies(&shell, &ctx));
  assert!(!RULE.applies(&other, &ctx));
}

#[test]
fn reviewed_head_does_not_authorize_other_or_implicit_pushes() {
  use crate::docs_review::tests::{fixture, report};
  let (dir, roots) = fixture();
  let repo = dir.path();
  let state = report(&roots, repo);
  crate::docs_review::write(&roots, repo, None, &state).unwrap();
  std::fs::create_dir_all(repo.join(".claude")).unwrap();
  std::fs::write(
    repo.join(".claude/toolu.config.json"),
    r#"{"version":1,"docsSync":{"mode":"block"}}"#,
  )
  .unwrap();
  for (command, allowed) in [
    ("git push origin HEAD:feature", true),
    ("git push origin main", true),
    ("git push origin", false),
    ("git push --mirror origin", false),
  ] {
    let raw = serde_json::json!({"tool_input":{"command":command}})
      .as_object()
      .unwrap()
      .clone();
    let ctx = RuleContext {
      host: Host::Claude,
      env: roots.env(),
      config_root: repo,
      project_root: repo,
      cwd: Some(repo),
      plugin_root: None,
      raw: &raw,
      edit: None,
    };
    assert_eq!(
      matches!(RULE.run(&push_event(command), &ctx), Decision::Allow),
      allowed,
      "{command}"
    );
  }
}

#[test]
fn possible_pushes_follow_the_target_repositorys_mode() {
  use crate::ledger::context::test_repo::Repo;
  let session = Repo::new().unwrap();
  let target = Repo::new().unwrap();
  target.sh("git config alias.p push").unwrap();
  let mode = |repo: &Repo, mode: &str| {
    let config = format!(r#"{{"version":1,"docsSync":{{"mode":"{mode}"}}}}"#);
    repo
      .sh(&format!(
        "mkdir -p .claude && printf '%s' '{config}' > .claude/toolu.config.json"
      ))
      .unwrap();
  };
  mode(&session, "block");
  mode(&target, "off");
  let env = session.env();
  let aliased = format!("git -C '{}' p origin main", target.root.display());
  assert!(matches!(
    run_in(&session.root, &env, &aliased),
    Decision::Allow
  ));
  mode(&target, "block");
  let Decision::Deny { reason } = run_in(&session.root, &env, &aliased) else {
    panic!("the target repository blocks");
  };
  assert!(reason.as_str().contains("may push"), "{}", reason.as_str());
  assert!(matches!(
    run_in(&session.root, &env, "git \"$sub\" log"),
    Decision::Deny { .. }
  ));
  mode(&session, "off");
  assert!(matches!(
    run_in(&session.root, &env, "git \"$sub\" log"),
    Decision::Allow
  ));
}

fn run_in(repo: &std::path::Path, env: &Env, command: &str) -> Decision {
  let raw = serde_json::json!({"tool_input":{"command":command}})
    .as_object()
    .unwrap()
    .clone();
  let ctx = RuleContext {
    host: Host::Claude,
    env,
    config_root: repo,
    project_root: repo,
    cwd: Some(repo),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  RULE.run(&push_event(command), &ctx)
}

#[test]
fn a_missing_review_names_the_skill() {
  use crate::docs_review::tests::fixture;
  let (dir, roots) = fixture();
  let repo = dir.path();
  strict(repo);
  let Decision::Deny { reason } = run_in(repo, roots.env(), "git push origin HEAD:feature") else {
    panic!("an unreviewed diff must be denied");
  };
  assert!(
    reason
      .as_str()
      .contains("no review is recorded for this diff; run /documentation-gate:sync"),
    "{}",
    reason.as_str()
  );
}

fn strict(repo: &std::path::Path) {
  std::fs::create_dir_all(repo.join(".claude")).unwrap();
  std::fs::write(
    repo.join(".claude/toolu.config.json"),
    r#"{"version":1,"gates":{"preset":"strict"}}"#,
  )
  .unwrap();
}

#[test]
fn only_a_lone_push_of_commits_already_in_base_skips_the_review() {
  use crate::docs_review::tests::{fixture, git};
  let (dir, roots) = fixture();
  let repo = dir.path();
  strict(repo);
  git(repo, &["update-ref", "refs/tags/v1.0", "main"]);
  git(repo, &["update-ref", "refs/tags/v2.0", "feature"]);
  let decide = |command: &str, allowed: bool| {
    assert_eq!(
      matches!(run_in(repo, roots.env(), command), Decision::Allow),
      allowed,
      "{command}"
    );
  };
  decide("git push", false);
  decide("git push origin v2.0", false);
  git(repo, &["checkout", "-q", "-b", "fresh", "main"]);
  decide("git push -u origin HEAD", true);
  decide(
    "git add -A && git commit -m x && git push -u origin HEAD",
    false,
  );
  decide("cd .. && git push origin HEAD:fresh", false);
  git(repo, &["checkout", "-q", "main"]);
  for command in ["git push", "git push origin main", "git push origin v1.0"] {
    decide(command, true);
  }
  decide("git push origin feature", false);
  decide("git push origin v2.0", false);
  git(repo, &["checkout", "-q", "--detach", "main"]);
  decide("git push origin HEAD", true);
}
