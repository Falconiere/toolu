use crate::delegation::delegate_command;
use toolu_runtime::env::Env;

fn trust(
  env: &Env,
  agent: &str,
  path: &str,
  repo: Option<&str>,
) -> Result<serde_json::Value, String> {
  let mut argv = vec!["delegate", "trust", "--agent", agent, "--path", path];
  if let Some(repo) = repo {
    argv.extend(["--repo", repo]);
  }
  let matches = delegate_command()
    .try_get_matches_from(argv)
    .map_err(|e| e.to_string())?;
  super::run(matches.subcommand_matches("trust").unwrap(), env)
}

#[test]
fn claude_trust_merges_into_the_existing_project_entry() {
  let home = tempfile::tempdir().unwrap();
  let env = Env::default().with("HOME", home.path().to_str().unwrap());
  let file = home.path().join(".claude.json");
  std::fs::write(
    &file,
    r#"{"theme":"dark","projects":{"/srv/w/tasks/a":{"allowedTools":["Bash"]}}}"#,
  )
  .unwrap();
  let reply = trust(&env, "claude", "/srv/w/tasks/a", Some("/srv/w/repos/h")).unwrap();
  assert_eq!(
    reply["trusted"],
    serde_json::json!(["/srv/w/tasks/a", "/srv/w/repos/h"])
  );
  let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
  assert_eq!(doc["theme"], "dark");
  assert_eq!(
    doc["projects"]["/srv/w/tasks/a"]["allowedTools"],
    serde_json::json!(["Bash"])
  );
  assert_eq!(
    doc["projects"]["/srv/w/tasks/a"]["hasTrustDialogAccepted"],
    true
  );
  assert_eq!(
    doc["projects"]["/srv/w/repos/h"]["hasTrustDialogAccepted"],
    true
  );
  let settings: serde_json::Value =
    serde_json::from_slice(&std::fs::read(home.path().join(".claude/settings.json")).unwrap())
      .unwrap();
  assert_eq!(
    settings,
    serde_json::json!({"skipDangerousModePermissionPrompt": true})
  );
  std::fs::write(&file, "[]").unwrap();
  assert!(trust(&env, "claude", "/srv/w/tasks/a", None).is_err());
  assert!(trust(&env, "claude", "relative", None).is_err());
}

#[test]
fn claude_trust_follows_claude_config_dir_and_keeps_other_settings() {
  let home = tempfile::tempdir().unwrap();
  let config = home.path().join("claude-config");
  std::fs::create_dir_all(&config).unwrap();
  let env = Env::default()
    .with("HOME", home.path().to_str().unwrap())
    .with("CLAUDE_CONFIG_DIR", config.to_str().unwrap());
  std::fs::write(config.join("settings.json"), "\n").unwrap();
  trust(&env, "claude", "/srv/w/tasks/b", None).unwrap();
  std::fs::write(config.join("settings.json"), r#"{"model":"opus"}"#).unwrap();
  let reply = trust(&env, "claude", "/srv/w/tasks/b", None).unwrap();
  assert_eq!(
    reply["store"],
    serde_json::json!([config.join(".claude.json"), config.join("settings.json")])
  );
  let doc: serde_json::Value =
    serde_json::from_slice(&std::fs::read(config.join(".claude.json")).unwrap()).unwrap();
  assert_eq!(
    doc,
    serde_json::json!({"projects":{"/srv/w/tasks/b":{"hasTrustDialogAccepted":true}}})
  );
  let settings: serde_json::Value =
    serde_json::from_slice(&std::fs::read(config.join("settings.json")).unwrap()).unwrap();
  assert_eq!(
    settings,
    serde_json::json!({"model":"opus","skipDangerousModePermissionPrompt":true})
  );
  assert!(!home.path().join(".claude.json").exists());
}

#[test]
fn codex_trust_appends_missing_tables_and_keeps_the_file_otherwise() {
  let home = tempfile::tempdir().unwrap();
  let codex = home.path().join("codex-home");
  let env = Env::default()
    .with("HOME", home.path().to_str().unwrap())
    .with("CODEX_HOME", codex.to_str().unwrap());
  trust(&env, "codex", "/srv/w/tasks/a", Some("/srv/w/repos/h")).unwrap();
  let file = codex.join("config.toml");
  let text = std::fs::read_to_string(&file).unwrap();
  assert_eq!(
    text,
    "\n[projects.\"/srv/w/tasks/a\"]\ntrust_level = \"trusted\"\n\n[projects.\"/srv/w/repos/h\"]\ntrust_level = \"trusted\"\n"
  );
  std::fs::write(
    &file,
    "# mine\nmodel = \"gpt-5\"\n[projects.\"/srv/w/repos/h\"]\ntrust_level = \"untrusted\"",
  )
  .unwrap();
  trust(&env, "codex", "/srv/w/tasks/a", Some("/srv/w/repos/h")).unwrap();
  let text = std::fs::read_to_string(&file).unwrap();
  assert!(text.starts_with("# mine\nmodel = \"gpt-5\"\n[projects.\"/srv/w/repos/h\"]\ntrust_level = \"untrusted\"\n\n[projects.\"/srv/w/tasks/a\"]\ntrust_level = \"trusted\"\n"));
  assert_eq!(text.matches("[projects.").count(), 2);
  let parsed: toml::Table = toml::from_str(&text).unwrap();
  assert_eq!(
    parsed["projects"]["/srv/w/tasks/a"]["trust_level"].as_str(),
    Some("trusted")
  );
  assert!(trust(&env, "muse", "/srv/w/tasks/a", None).is_err());
}
