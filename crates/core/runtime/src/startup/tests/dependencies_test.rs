use std::os::unix::fs::PermissionsExt as _;

use toolu_protocol::host::Host;

use super::{
  NoticeStyle, codex_dependency_notice, codex_missing_plugins, requires_core_warning,
  requires_plugins_warning,
};
use crate::env::Env;
use crate::host::roots::Roots;

/// Codex roots whose `codex` prints `listing`, or exits 1 when it is `None`.
fn codex(dir: &tempfile::TempDir, listing: Option<&str>) -> Roots {
  let script = dir.path().join("codex");
  let body = listing.map_or_else(
    || "exit 1".to_owned(),
    |text| format!("cat <<'JSON'\n{text}\nJSON"),
  );
  std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!(
    "{}:{}",
    dir.path().display(),
    std::env::var("PATH").unwrap()
  );
  Roots::new(
    Env::from_pairs([("PATH", path.as_str()), ("PLUGIN_ROOT", "/p")]),
    None,
  )
}

#[test]
fn installed_and_enabled_plugins_are_not_missing() {
  let dir = tempfile::tempdir().unwrap();
  let listing =
    r#"{"installed":[null,{"pluginId":"toolu@toolu"},{"pluginId":"jev@toolu","enabled":false}]}"#;
  let roots = codex(&dir, Some(listing));
  assert_eq!(roots.host(), Host::Codex);
  let missing = codex_missing_plugins(&["toolu@toolu", "jev@toolu"], &roots);
  assert_eq!(missing, Some(vec!["jev@toolu".to_owned()]));
  assert_eq!(
    codex_dependency_notice(&["toolu@toolu"], NoticeStyle::Core, &roots),
    None
  );
}

#[test]
fn an_object_listing_counts_and_an_unindexable_entry_stops_the_search() {
  let dir = tempfile::tempdir().unwrap();
  let map = codex(
    &dir,
    Some(r#"{"installed":{"a":{"pluginId":"toolu@toolu","installed":true}}}"#),
  );
  assert_eq!(
    codex_missing_plugins(&["toolu@toolu"], &map),
    Some(Vec::new())
  );
  let raised = codex(
    &dir,
    Some(r#"{"installed":["text",{"pluginId":"toolu@toolu"}]}"#),
  );
  assert_eq!(
    codex_missing_plugins(&["toolu@toolu"], &raised),
    Some(vec!["toolu@toolu".to_owned()])
  );
  let garbage = codex(&dir, Some("not json"));
  assert_eq!(
    codex_missing_plugins(&["a@b", "c@d"], &garbage),
    Some(vec!["a@b".to_owned(), "c@d".to_owned()])
  );
}

#[test]
fn the_notice_is_the_pretty_session_context() {
  let dir = tempfile::tempdir().unwrap();
  let roots = codex(&dir, Some(r#"{"installed":[]}"#));
  let core = codex_dependency_notice(&["toolu@toolu"], NoticeStyle::Core, &roots).unwrap();
  let text = "WARN: this plugin requires the toolu core. Install it first with: codex plugin add toolu@toolu";
  let expected = format!(
    "{{\n  \"hookSpecificOutput\": {{\n    \"hookEventName\": \"SessionStart\",\n    \"additionalContext\": \"{text}\"\n  }}\n}}\n"
  );
  assert_eq!(core, expected);
  assert_eq!(requires_core_warning(), text);
  let each = codex_dependency_notice(&["a@b", "c@d"], NoticeStyle::Each, &roots).unwrap();
  let wanted = requires_plugins_warning(&["a@b".to_owned(), "c@d".to_owned()]);
  assert_eq!(
    wanted,
    "WARN: this plugin requires a@b (install with: codex plugin add a@b) c@d (install with: codex plugin add c@d)"
  );
  assert!(each.contains(&wanted));
}

#[test]
fn nothing_is_checked_off_codex_without_plugin_root_or_without_the_cli() {
  let dir = tempfile::tempdir().unwrap();
  let roots = codex(&dir, Some(r#"{"installed":[]}"#));
  let claude = Roots::new(roots.env().clone(), Some(Host::Claude));
  assert_eq!(codex_missing_plugins(&["toolu@toolu"], &claude), None);
  let unrooted = Roots::new(
    roots.env().clone().with("PLUGIN_ROOT", ""),
    Some(Host::Codex),
  );
  assert_eq!(codex_missing_plugins(&["toolu@toolu"], &unrooted), None);
  let failing = codex(&dir, None);
  assert_eq!(codex_missing_plugins(&["toolu@toolu"], &failing), None);
}
