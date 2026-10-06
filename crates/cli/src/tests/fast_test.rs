use super::{Fast, HookRequest, command, parse, request};

fn parsed(line: &str) -> Option<Fast> {
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
    Some(Fast::Hook(HookRequest {
      plugin: "toolu".to_owned(),
      name: "pre-tools".to_owned(),
      event: Some("PreToolUse".to_owned()),
      plugin_root: Some("/p".to_owned()),
    }))
  );
  assert_eq!(
    parsed("jev hook session-start"),
    Some(Fast::Hook(HookRequest {
      plugin: "jev".to_owned(),
      name: "session-start".to_owned(),
      event: None,
      plugin_root: None,
    }))
  );
  assert_eq!(parsed("--hook-protocol"), Some(Fast::HookProtocol));
  let Some(Fast::Hook(review)) = parsed("toolu-review hook check --event Stop") else {
    panic!("not a hook")
  };
  assert_eq!(review.plugin, "toolu-review");
  let Some(Fast::Hook(relative)) = parsed("hook x --plugin-root ./--my-dir") else {
    panic!("not a hook")
  };
  assert_eq!(relative.plugin_root.as_deref(), Some("./--my-dir"));
  let Some(Fast::Hook(newer)) = parsed("newer-plugin hook check --event Stop") else {
    panic!("not a hook")
  };
  assert_eq!(newer.plugin, "newer-plugin");
}

#[test]
fn an_empty_plugin_root_is_kept_not_dropped() {
  let words = ["hook", "x", "--plugin-root", ""].map(str::to_owned);
  let Some(Fast::Hook(request)) = parse(&words) else {
    panic!("not a hook")
  };
  assert_eq!(request.plugin_root.as_deref(), Some(""));
}

#[test]
fn everything_else_is_left_to_clap() {
  for line in [
    "",
    "--version",
    "--help",
    "--hook-protocol --json",
    "hook",
    "hook --help",
    "hook --event X",
    "hook x --event",
    "hook x --nope y",
    "--x hook y",
    "-toolu hook y",
    "Bad_Name hook y",
    "hook Bad_Name",
    "jev hook a--b",
    "epic planned",
    "a b c",
    "review hook x --event Stop",
    "commands hook x",
    "ledger hook x",
    "toolu hook x",
    "hook x --event --plugin-root",
    "hook x --plugin-root -foo",
    "hook x --event A --event B",
    "jev hook x --plugin-root /a --plugin-root /b",
  ] {
    assert_eq!(parsed(line), None, "{line}");
  }
}

#[test]
fn the_clap_mirror_parses_what_the_fast_path_parses() {
  let matches = command()
    .try_get_matches_from([
      "hook",
      "session-start",
      "--event",
      "SessionStart",
      "--plugin-root",
      "/p",
    ])
    .unwrap();
  assert_eq!(
    request("jev", &matches),
    HookRequest {
      plugin: "jev".to_owned(),
      name: "session-start".to_owned(),
      event: Some("SessionStart".to_owned()),
      plugin_root: Some("/p".to_owned()),
    }
  );
}

#[test]
fn the_clap_mirror_rejects_what_the_fast_path_declines() {
  for argv in [
    &["hook"][..],
    &["hook", "Bad_Name"],
    &["hook", "x", "--nope", "y"],
    &["hook", "x", "--event"],
    &["hook", "x", "--event", "A", "--event", "B"],
    &["hook", "x", "--event", "--plugin-root"],
    &["hook", "x", "--plugin-root", "-foo"],
  ] {
    assert!(command().try_get_matches_from(argv).is_err(), "{argv:?}");
  }
}
