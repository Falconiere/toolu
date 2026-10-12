use std::os::unix::fs::PermissionsExt;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::check_toolu;

#[test]
fn codex_without_the_core_is_warned_and_other_hosts_are_silent() {
  let dir = tempfile::tempdir().unwrap();
  let stub = dir.path().join("codex");
  let plugins = r#"{"installed":[{"pluginId":"ts-quality@toolu"}]}"#;
  std::fs::write(&stub, format!("#!/bin/sh\nprintf '%s\\n' '{plugins}'\n")).unwrap();
  std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
  let search = format!("{}:/usr/bin:/bin", dir.path().display());
  let codex = Env::from_pairs([
    ("PATH", search.as_str()),
    ("CODEX_HOME", "/c"),
    ("PLUGIN_ROOT", "/p"),
  ]);
  let warned = check_toolu(&codex);
  assert_eq!(warned.exit, Exit::Success);
  let text = warned.stdout.unwrap_or_default();
  assert!(
    text.contains("toolu@toolu") && !text.ends_with('\n'),
    "{text}"
  );
  let claude = Env::from_pairs([("CLAUDE_PLUGIN_ROOT", "/p")]);
  assert_eq!(check_toolu(&claude).stdout, None);
}
