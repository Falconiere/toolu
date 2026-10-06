use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::StateCtx;

fn roots(home: &std::path::Path, project: &std::path::Path) -> Roots {
  let env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_PROJECT_DIR", project.display().to_string()),
  ]);
  Roots::new(env, Some(Host::Claude))
}

#[test]
fn the_clock_is_fixed_when_set_and_the_system_clock_otherwise() {
  let dir = tempfile::tempdir().unwrap();
  let mut ctx = StateCtx::new(roots(dir.path(), dir.path()));
  let before = SystemTime::now();
  assert!(ctx.now() >= before);
  let fixed = UNIX_EPOCH + Duration::from_secs(42);
  ctx.now = Some(fixed);
  assert_eq!(ctx.now(), fixed);
}

#[test]
fn the_config_is_loaded_once_and_its_warnings_are_collected() {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("p");
  std::fs::create_dir_all(project.join(".claude")).unwrap();
  std::fs::write(project.join(".claude/toolu.config.json"), "{oops").unwrap();
  let mut ctx = StateCtx::new(roots(dir.path(), &project));
  assert!(ctx.config(&project).data.is_empty());
  assert_eq!(ctx.warnings.len(), 1);
  assert!(
    ctx.warnings[0].starts_with("malformed JSON in "),
    "{:?}",
    ctx.warnings
  );
  std::fs::write(project.join(".claude/toolu.config.json"), "{\"version\":1}").unwrap();
  assert!(
    ctx.config(&project).data.is_empty(),
    "loaded once per context"
  );
  assert_eq!(ctx.warnings.len(), 1);
}
