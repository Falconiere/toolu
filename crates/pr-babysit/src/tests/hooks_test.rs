use std::os::unix::fs::PermissionsExt;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{check_binary, check_toolu};

fn codex_without_core(dir: &tempfile::TempDir) -> Env {
  let codex = dir.path().join("codex");
  let listing = r#"{"installed":[{"pluginId":"pr-babysit@toolu"}]}"#;
  std::fs::write(
    &codex,
    format!("#!/bin/sh\ncat <<'JSON'\n{listing}\nJSON\n"),
  )
  .unwrap();
  std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!("{}:/usr/bin:/bin", dir.path().display());
  Env::from_pairs([
    ("PATH", path.as_str()),
    ("CODEX_HOME", "/c"),
    ("PLUGIN_ROOT", "/p"),
  ])
}

#[test]
fn check_toolu_warns_codex_without_the_core_and_is_silent_elsewhere() {
  let dir = tempfile::tempdir().unwrap();
  let warned = check_toolu(&codex_without_core(&dir));
  assert_eq!(warned.exit, Exit::Success);
  let text = warned.stdout.unwrap();
  assert!(text.contains("toolu@toolu"), "{text}");
  assert!(!text.ends_with('\n'));
  let claude = Env::from_pairs([("CLAUDE_PLUGIN_ROOT", "/p")]);
  assert_eq!(check_toolu(&claude).stdout, None);
}

#[test]
fn check_binary_never_fails_the_session() {
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  let outcome = check_binary(&env, Some(r#"{"session_id":"s"}"#));
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stderr, None);
}
