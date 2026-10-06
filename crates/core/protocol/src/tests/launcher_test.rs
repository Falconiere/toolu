use super::{DEFAULT_TIMEOUT, MARKER, MAX_TIMEOUT, Target, hook, hook_name};

const PRE_TOOL_USE: &str = include_str!("fixtures/launcher-pre-tool-use.txt");
const SESSION_START: &str = include_str!("fixtures/launcher-session-start.txt");
const SESSION_HOOK: &str =
  include_str!("../../../../../tooling/fixtures/native-launcher/session-start.json");
const PRE_HOOK: &str =
  include_str!("../../../../../tooling/fixtures/native-launcher/pre-tool-use.json");

fn target<'a>(plugin: &'a str, event: &'a str, name: &'a str) -> Target<'a> {
  Target {
    plugin,
    event,
    name,
  }
}

fn generated(plugin: &str, event: &str, name: &str) -> super::LauncherHook {
  hook(&target(plugin, event, name), DEFAULT_TIMEOUT).unwrap()
}

#[test]
fn enforcing_and_context_commands_match_the_committed_goldens() {
  assert_eq!(
    generated("toolu", "PreToolUse", "pre-tools").command,
    PRE_TOOL_USE
  );
  assert_eq!(
    generated("toolu", "SessionStart", "session-start").command,
    SESSION_START
  );
}

#[test]
fn typescript_checker_fixtures_match_the_rust_generator() {
  for (plugin, event, name, fixture) in [
    ("toolu", "SessionStart", "session-start", SESSION_HOOK),
    ("toolu", "PreToolUse", "pre-tools", PRE_HOOK),
  ] {
    let generated = generated(plugin, event, name);
    let expected = serde_json::json!({
      "type": "command",
      "command": generated.command,
      "commandWindows": generated.command_windows,
      "timeout": generated.timeout,
    });
    let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(fixture, expected, "{plugin} {event} {name}");
  }
}

#[test]
fn an_enforcing_entry_runs_without_exec_and_maps_other_statuses_to_2() {
  let command = generated("toolu", "PermissionRequest", "permission").command;
  assert!(command.contains("then \"$t\" hook permission --event PermissionRequest"));
  assert!(command.contains("case $s in 0|2) exit $s;; esac;"));
  assert!(command.ends_with("' >&2; exit 2"));
  assert!(!command.contains("exec \"$t\""));
}

#[test]
fn a_context_entry_execs_and_ends_with_a_system_message() {
  let command = generated("toolu", "PostToolUse", "post-tools").command;
  assert!(command.contains("then exec \"$t\" hook post-tools --event PostToolUse"));
  assert!(command.ends_with("\"}'; exit 0"));
}

#[test]
fn another_plugin_runs_its_own_namespace_and_bundle() {
  let command = generated("jev", "SessionStart", "session-start").command;
  assert!(command.contains("\"$t\" jev hook session-start --event SessionStart"));
  assert!(command.contains("${CLAUDE_PLUGIN_ROOT}/hooks/dist/session-start.js"));
  assert!(command.contains("'{\"systemMessage\":\"jev plugin: toolu is not installed"));
}

#[test]
fn the_command_carries_no_version_and_names_every_install_directory_in_order() {
  let command = generated("toolu", "SessionStart", "session-start").command;
  assert!(!command.contains(env!("CARGO_PKG_VERSION")));
  let order = [
    "\"$TOOLU_BIN\"",
    "/opt/homebrew/bin/toolu",
    "/usr/local/bin/toolu",
    "/home/linuxbrew/.linuxbrew/bin/toolu",
    "\"$HOME/.local/bin/toolu\"",
    "command -v toolu",
  ];
  let positions: Vec<usize> = order
    .iter()
    .map(|item| command.find(item).unwrap())
    .collect();
  assert!(
    positions.windows(2).all(|pair| pair[0] < pair[1]),
    "{positions:?}"
  );
  assert!(command.contains(MARKER));
}

#[test]
fn windows_keeps_the_bun_chain_and_escapes_the_installer_pipe() {
  let windows = generated("toolu", "PreToolUse", "pre-tools").command_windows;
  assert!(windows.starts_with("if exist \"%TOOLU_BUN%\" (\"%TOOLU_BUN%\" "));
  assert!(windows.contains("\"%PLUGIN_ROOT%\\hooks\\dist\\pre-tools.js\""));
  assert!(windows.contains("install ^| bash"));
  assert_eq!(windows.matches('|').count(), 1, "{windows}");
  assert!(windows.ends_with("See docs/install.md.& exit /b 2))"));
  let context = generated("toolu", "SessionStart", "session-start").command_windows;
  assert!(context.ends_with("See docs/install.md.\"}))"));
}

#[test]
fn bad_names_events_and_timeouts_are_rejected() {
  for (plugin, event, name) in [
    ("Bad_Name", "PreToolUse", "pre-tools"),
    ("toolu", "PreToolUse", "Bad_Name"),
    ("toolu", "PreToolUse", ""),
    ("toolu", "PreToolUse", "a/b"),
    ("toolu", "PreToolUse", "-a"),
    ("toolu", "PreToolUse", "a--b"),
    ("toolu", "preToolUse", "pre-tools"),
    ("toolu", "P", "pre-tools"),
    ("toolu", "Pre Tool", "pre-tools"),
  ] {
    assert!(
      hook(&target(plugin, event, name), DEFAULT_TIMEOUT).is_err(),
      "{plugin} {event} {name}"
    );
  }
  let ok = target("toolu", "PreToolUse", "pre-tools");
  assert!(hook(&ok, 0).unwrap_err().contains("1 to 600"));
  assert!(hook(&ok, MAX_TIMEOUT + 1).is_err());
  assert_eq!(hook(&ok, MAX_TIMEOUT).unwrap().timeout, MAX_TIMEOUT);
  assert_eq!(hook(&ok, 1).unwrap().timeout, 1);
}

#[test]
fn the_hook_name_is_the_word_after_the_first_hook() {
  assert_eq!(hook_name(PRE_TOOL_USE), Some("pre-tools"));
  assert_eq!(hook_name(SESSION_START), Some("session-start"));
  let jev = generated("jev", "SessionStart", "session-start").command;
  assert_eq!(hook_name(&jev), Some("session-start"));
  assert_eq!(hook_name("toolu --hook-protocol"), None);
  assert_eq!(hook_name("x hook Bad_Name --event E"), None);
  assert_eq!(hook_name("x hook "), None);
}
