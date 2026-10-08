use super::*;

#[test]
fn malformed_payload_is_silent() {
  assert_eq!(
    evaluate("[1]", None, Env::process(), Path::new(".")),
    silent()
  );
}

fn fixture(host: &str) -> (tempfile::TempDir, Env) {
  let dir = tempfile::tempdir().expect("tempdir");
  let project = dir.path().join("project");
  let settings = dir.path().join("settings");
  std::fs::create_dir_all(&project).expect("project");
  std::fs::create_dir_all(&settings).expect("settings");
  std::fs::write(
    settings.join("mcp-blocklist.txt"),
    "exampleblocked -> use CLI\n",
  )
  .expect("blocklist");
  let env = Env::process()
    .with("TOOLU_HOST_OVERRIDE", host)
    .with("TOOLU_PROJECT_DIR", &project.display().to_string())
    .with(
      "TOOLU_CONFIG_DIR",
      &dir.path().join("user").display().to_string(),
    )
    .with("TOOLU_SETTINGS_DIR", &settings.display().to_string());
  (dir, env)
}

#[test]
fn real_blocklist_asks_on_claude_and_denies_on_codex() {
  let payload = r#"{"tool_name":"mcp__exampleblocked__search","tool_input":{}}"#;
  for (host, action) in [("claude", "ask"), ("codex", "deny")] {
    let (dir, env) = fixture(host);
    let out = evaluate(payload, None, env, &dir.path().join("project"));
    assert_eq!(out.exit, Exit::Success);
    assert_eq!(out.stderr, None);
    let json: Value = serde_json::from_str(&out.stdout.expect("decision")).expect("json");
    assert_eq!(json["hookSpecificOutput"]["permissionDecision"], action);
    assert!(
      json["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("reason")
        .contains("Use instead: use CLI")
    );
  }
}

#[test]
fn malformed_project_config_warns_and_keeps_blocklist_decision() {
  let (dir, env) = fixture("claude");
  let config = dir.path().join("project/.claude/toolu.config.json");
  std::fs::create_dir_all(config.parent().expect("parent")).expect("config dir");
  std::fs::write(&config, "{not json").expect("config");
  let payload = r#"{"tool_name":"mcp__exampleblocked__search"}"#;
  let out = evaluate(payload, None, env, &dir.path().join("project"));
  assert!(out.stdout.expect("decision").contains("\"ask\""));
  assert!(
    out
      .stderr
      .expect("warning")
      .starts_with("toolu-config: malformed JSON in ")
  );
}
