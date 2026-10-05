use super::{Command, HookRequest, parse};

fn parsed(line: &str) -> Result<Command, String> {
  let words: Vec<String> = line
    .split(' ')
    .filter(|w| !w.is_empty())
    .map(str::to_owned)
    .collect();
  parse(&words)
}

#[test]
fn the_toolu_and_plugin_forms_parse() {
  assert_eq!(
    parsed("hook pre-tools --event PreToolUse --plugin-root /p"),
    Ok(Command::Hook(HookRequest {
      plugin: "toolu".to_owned(),
      name: "pre-tools".to_owned(),
      event: Some("PreToolUse".to_owned()),
      plugin_root: Some("/p".to_owned()),
    }))
  );
  assert_eq!(
    parsed("jev hook session-start"),
    Ok(Command::Hook(HookRequest {
      plugin: "jev".to_owned(),
      name: "session-start".to_owned(),
      event: None,
      plugin_root: None,
    }))
  );
}

#[test]
fn an_empty_plugin_root_is_kept_not_dropped() {
  let words = ["hook", "x", "--plugin-root", ""].map(str::to_owned);
  let Ok(Command::Hook(request)) = parse(&words) else {
    panic!("not a hook")
  };
  assert_eq!(request.plugin_root.as_deref(), Some(""));
}

#[test]
fn flags_and_forms_outside_the_grammar_are_errors() {
  assert_eq!(parsed("--version"), Ok(Command::Version));
  assert_eq!(parsed("--hook-protocol"), Ok(Command::HookProtocol));
  for line in [
    "",
    "--help",
    "hook",
    "hook --event X",
    "hook x --event",
    "hook x --nope y",
    "--x hook y",
    "-toolu hook y",
    "Bad_Name hook y",
    "hook Bad_Name",
    "jev hook a--b",
    "a b c",
  ] {
    assert!(parsed(line).is_err(), "{line}");
  }
}
