//! The `toolu` CLI contract (#442), black-box: every namespace answers
//! `--help`, data goes to stdout and diagnostics to stderr, `--json` leaves one
//! document on stdout, and the exit codes are the documented ones. The schema
//! and snapshot of `toolu commands --json` and the plugin inventory are in the
//! helper modules.

#[path = "helpers/cli.rs"]
mod cli;
#[path = "helpers/inventory.rs"]
mod inventory;
#[path = "helpers/schema.rs"]
mod schema;

use cli::{namespaces, one_document, stderr, stdout, toolu};
use serde_json::json;

/// The visible namespaces that are not ported yet: all but the hook runner, the
/// command export and the two Markdown-only guides.
fn unported(names: &[String]) -> Vec<&String> {
  let ported = [
    "hook",
    "ledger",
    "commands",
    "doctor",
    "config",
    "status",
    "setup",
    "brainstorm",
    "delivery-flow",
  ];
  names
    .iter()
    .filter(|name| !ported.contains(&name.as_str()))
    .collect()
}

#[test]
fn every_namespace_answers_help_on_stdout() {
  let names = namespaces().unwrap();
  assert_eq!(
    names,
    [
      "hook",
      "ledger",
      "debug",
      "setup",
      "doctor",
      "config",
      "status",
      "serve",
      "ts-quality",
      "python-quality",
      "rust-quality",
      "ast-grep",
      "brainstorm",
      "delivery-flow",
      "review",
      "jev",
      "statusline",
      "babysit",
      "epic",
      "commands",
      "plugins",
    ]
  );
  for name in &names {
    let output = toolu(&[name, "--help"]).unwrap();
    assert_eq!(output.status.code(), Some(0), "{name}");
    let help = stdout(&output).unwrap();
    assert!(
      help.contains(&format!("Usage: toolu {name}")),
      "{name}: {help}"
    );
    assert_eq!(stderr(&output).unwrap(), "", "{name}");
  }
  for name in unported(&names) {
    let help = stdout(&toolu(&[name, "--help"]).unwrap()).unwrap();
    assert!(
      help.contains("\n  planned  Not ported yet (#"),
      "{name}: {help}"
    );
  }
  let epic = stdout(&toolu(&["epic", "--help"]).unwrap()).unwrap();
  assert!(
    epic.contains("planned  Not ported yet (#435, #448)"),
    "{epic}"
  );
}

#[test]
fn help_and_version_are_data_on_stdout() {
  let help = toolu(&["--help"]).unwrap();
  assert_eq!(help.status.code(), Some(0));
  assert!(stdout(&help).unwrap().contains("Exit codes:"));
  assert_eq!(stderr(&help).unwrap(), "");
  let version = toolu(&["--version"]).unwrap();
  assert_eq!(version.status.code(), Some(0));
  assert_eq!(
    stdout(&version).unwrap(),
    format!("toolu {}\n", env!("CARGO_PKG_VERSION"))
  );
  for args in [&["--json", "--help"][..], &["--version", "--json"]] {
    let output = toolu(args).unwrap();
    assert_eq!(output.status.code(), Some(0), "{args:?}");
    one_document(&output).unwrap();
    assert_eq!(stderr(&output).unwrap(), "", "{args:?}");
  }
}

#[test]
fn a_typo_exits_64_and_suggests_the_namespace_on_stderr() {
  let output = toolu(&["epik", "start"]).unwrap();
  assert_eq!(output.status.code(), Some(64));
  assert_eq!(stdout(&output).unwrap(), "");
  let diagnostic = stderr(&output).unwrap();
  assert!(
    diagnostic.contains("tip: a similar subcommand exists: 'epic'"),
    "{diagnostic}"
  );
}

#[test]
fn json_on_a_usage_error_is_one_document_and_a_diagnostic() {
  let output = toolu(&["--json", "epik", "start"]).unwrap();
  assert_eq!(output.status.code(), Some(64));
  assert_eq!(
    one_document(&output).unwrap(),
    json!({ "error": { "code": 64, "name": "usage",
      "message": "unrecognized subcommand 'epik'", "suggestion": "epic" } })
  );
  assert!(
    stderr(&output)
      .unwrap()
      .starts_with("error: unrecognized subcommand 'epik'\n")
  );
}

#[test]
fn a_skill_running_epic_status_with_json_gets_one_document() {
  let output = toolu(&["epic", "status", "402", "--json"]).unwrap();
  assert_eq!(output.status.code(), Some(0));
  let document = one_document(&output).unwrap();
  assert!(document.get("engine").is_some(), "{document}");
  assert!(document.get("issues").is_some(), "{document}");
  assert_eq!(stderr(&output).unwrap(), "");
}

#[test]
fn a_missing_verb_exits_64_with_help_on_stderr() {
  let output = toolu(&["epic"]).unwrap();
  assert_eq!(output.status.code(), Some(64));
  assert_eq!(stdout(&output).unwrap(), "");
  assert!(stderr(&output).unwrap().contains("Usage: toolu epic"));
  assert_eq!(toolu(&[]).unwrap().status.code(), Some(64));
}

#[test]
fn a_denied_hook_exits_2_with_blocked_on_stderr() {
  let output = toolu(&["hook", "no-such-hook", "--event", "PreToolUse"]).unwrap();
  assert_eq!(output.status.code(), Some(2));
  assert_eq!(stdout(&output).unwrap(), "");
  assert!(
    stderr(&output)
      .unwrap()
      .starts_with("blocked: toolu plugin:")
  );
}

#[test]
fn a_successful_verb_leaves_stderr_empty() {
  for args in [
    &["epic", "planned"][..],
    &["--json", "jev", "planned"],
    &["brainstorm"],
    &["commands"],
  ] {
    let output = toolu(args).unwrap();
    assert_eq!(output.status.code(), Some(0), "{args:?}");
    assert_eq!(stderr(&output).unwrap(), "", "{args:?}");
  }
  let guide = stdout(&toolu(&["delivery-flow"]).unwrap()).unwrap();
  assert!(guide.contains("/delivery-flow:delivery-flow"), "{guide}");
}

#[test]
fn host_must_be_a_known_host_and_config_dir_is_accepted() {
  let bogus = toolu(&["--host", "bogus", "commands"]).unwrap();
  assert_eq!(bogus.status.code(), Some(64));
  assert_eq!(stdout(&bogus).unwrap(), "");
  assert!(
    stderr(&bogus)
      .unwrap()
      .contains("[possible values: claude, codex, opencode, cursor, hermes]")
  );
  let known = toolu(&["--host", "codex", "--config-dir", "/tmp", "commands"]).unwrap();
  assert_eq!(known.status.code(), Some(0));
  assert!(stdout(&known).unwrap().starts_with("toolu hook "));
  assert_eq!(stderr(&known).unwrap(), "");
}

#[test]
fn every_generated_hook_form_is_in_the_clap_tree() {
  for args in [
    &["hook", "--help"][..],
    &["jev", "hook", "--help"],
    &["pr-babysit", "hook", "--help"],
    &["epic-orchestrator", "hook", "--help"],
  ] {
    let output = toolu(args).unwrap();
    assert_eq!(output.status.code(), Some(0), "{args:?}");
    assert!(
      stdout(&output).unwrap().contains("--plugin-root <DIR>"),
      "{args:?}"
    );
  }
  let alias = toolu(&["toolu-review", "planned"]).unwrap();
  assert!(
    stdout(&alias)
      .unwrap()
      .starts_with("toolu review is not ported yet")
  );
}

#[test]
fn a_plugin_newer_than_the_binary_gets_the_upgrade_advice_not_a_usage_error() {
  let root = tempfile::tempdir().unwrap();
  let manifest = root.path().join(".claude-plugin");
  std::fs::create_dir_all(&manifest).unwrap();
  std::fs::write(
    manifest.join("plugin.json"),
    r#"{"name":"newer-plugin","version":"0.0.1","hookProtocol":99}"#,
  )
  .unwrap();
  let plugin_root = root.path().to_str().unwrap();
  for (event, code) in [("SessionStart", 0), ("PreToolUse", 2)] {
    let args = [
      "newer-plugin",
      "hook",
      "x",
      "--event",
      event,
      "--plugin-root",
      plugin_root,
    ];
    let output = toolu(&args).unwrap();
    assert_eq!(output.status.code(), Some(code), "{event}");
    let shown = format!("{}{}", stdout(&output).unwrap(), stderr(&output).unwrap());
    assert!(
      shown.contains("newer-plugin plugin: hook protocol 99 needs a newer toolu"),
      "{event}: {shown}"
    );
  }
}
