use super::*;

#[test]
fn registration_is_idempotent_and_rejects_invalid_manifest() {
  let dir = tempfile::tempdir().expect("tempdir");
  let config = dir.path().to_str().expect("path");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", config)]);
  let roots = Roots::new(env.clone(), Some(Host::Claude));
  assert!(!active(&roots));
  register(&env, Host::Claude);
  assert!(active(&roots));
  let path = event_dir(&roots, RegistryEvent::ToolPre).join(FILE);
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
    ("git push origin main", false),
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
fn dynamic_push_indicators_follow_mode_without_blocking_other_dynamic_commands() {
  use crate::ledger::context::test_repo::Repo;
  let repo = Repo::new().unwrap();
  repo.sh("mkdir -p .claude").unwrap();
  let config = repo.root.join(".claude/toolu.config.json");
  let env = repo.env();
  for (mode, command, denied) in [
    ("block", "action=push; git \"$action\" origin main", true),
    ("block", "$git_command push origin main", true),
    ("block", "$formatter file", false),
    ("off", "action=push; git \"$action\" origin main", false),
    ("off", "$git_command push origin main", false),
  ] {
    std::fs::write(
      &config,
      format!(r#"{{"version":1,"docsSync":{{"mode":"{mode}"}}}}"#),
    )
    .unwrap();
    let raw = serde_json::json!({"tool_input":{"command":command}})
      .as_object()
      .unwrap()
      .clone();
    let ctx = RuleContext {
      host: Host::Claude,
      env: &env,
      config_root: &repo.root,
      project_root: &repo.root,
      cwd: Some(&repo.root),
      plugin_root: None,
      raw: &raw,
      edit: None,
    };
    assert_eq!(
      matches!(RULE.run(&push_event(command), &ctx), Decision::Deny { .. }),
      denied,
      "{command}"
    );
  }
}
