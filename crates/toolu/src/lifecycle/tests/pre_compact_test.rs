use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{PreCompactInput, pre_compact};

#[test]
fn a_megabyte_of_nuls_is_silent_even_when_the_hook_is_disabled() {
  let dir = tempfile::tempdir().unwrap();
  let cfg = dir.path().join("cfg");
  std::fs::create_dir_all(&cfg).unwrap();
  std::fs::write(
    cfg.join("toolu.config.json"),
    r#"{"hooks":{"pre-compact":false}}"#,
  )
  .unwrap();
  let payload = "\0".repeat(1024 * 1024);
  assert_eq!(payload.len(), 1024 * 1024);
  let env = Env::from_pairs([
    ("HOME", dir.path().join("home").to_str().unwrap()),
    ("PATH", "/usr/bin:/bin"),
    ("TOOLU_CONFIG_DIR", cfg.to_str().unwrap()),
    ("CLAUDE_PROJECT_DIR", dir.path().to_str().unwrap()),
  ]);
  assert!(payload.chars().all(|byte| byte == '\0'));
  let outcome = pre_compact(&PreCompactInput {
    env: &env,
    cwd: Path::new(dir.path()),
  });
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr, None);
}
