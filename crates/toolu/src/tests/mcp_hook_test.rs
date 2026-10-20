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

fn observed_muse_payload(tool: &str) -> String {
  let path = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
  );
  let inventory: Value =
    serde_json::from_str(&std::fs::read_to_string(path).expect("inventory")).expect("JSON");
  inventory["tools"][tool]["pre_example"].to_string()
}

#[test]
fn observed_muse_mcp_name_blocks_with_muse_json() {
  let (dir, env) = fixture("muse");
  std::fs::write(
    dir.path().join("settings/mcp-blocklist.txt"),
    "plugin_toolu_probe_540_probe_mcp -> use CLI\n",
  )
  .expect("blocklist");
  let payload = observed_muse_payload("mcp__plugin_toolu_probe_540_probe_mcp__probe_echo");
  let out = evaluate(&payload, None, env, &dir.path().join("project"));
  assert_eq!(out.exit, Exit::Success);
  assert_eq!(out.stderr, None);
  let stdout = out.stdout.expect("Muse block");
  let json: Value = serde_json::from_str(&stdout).expect("block JSON");
  assert_eq!(json["decision"], "block");
  assert!(json["reason"].as_str().expect("reason").contains("use CLI"));
  assert!(json.get("hookSpecificOutput").is_none());
}

#[test]
fn unmatched_muse_mcp_and_internal_tool_are_silent() {
  let (dir, env) = fixture("muse");
  let mcp = observed_muse_payload("mcp__plugin_toolu_probe_540_probe_mcp__probe_echo");
  let internal = observed_muse_payload("submit_reminder_decision");
  for payload in [mcp, internal] {
    assert_eq!(
      evaluate(&payload, None, env.clone(), &dir.path().join("project")),
      silent()
    );
  }
}
