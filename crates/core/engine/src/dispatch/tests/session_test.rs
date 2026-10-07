use std::path::Path;

use toolu_runtime::env::Env;

use super::{Opened, Session, post_env};
use crate::dispatch::{DEFAULT_MODULE_TIMEOUT, DispatchOptions, Phase};

fn options<'a>(env: &'a Env, cwd: &'a Path) -> DispatchOptions<'a> {
  DispatchOptions {
    env,
    cwd,
    lib_dir: Path::new("/lib"),
    builtins: &[],
    rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  }
}

#[test]
fn after_a_tool_modules_see_the_project_root_and_its_bin_first() {
  let dir = tempfile::tempdir().unwrap();
  let cwd = std::fs::canonicalize(dir.path()).unwrap();
  let env = Env::from_pairs([("PATH", "/usr/bin")]);
  let (root, env) = post_env(&env, &cwd, "/cfg");
  let root_text = root.to_string_lossy().into_owned();
  assert_eq!(root, cwd, "outside a repository the root is the cwd");
  assert_eq!(env.get("PROJECT_ROOT"), Some(root_text.as_str()));
  assert_eq!(
    env.get("PATH"),
    Some(format!("{root_text}/node_modules/.bin:/usr/bin").as_str())
  );
  assert_eq!(env.get("TOOLU_CONFIG_DIR"), Some("/cfg"));
}

#[test]
fn a_hook_switched_off_keeps_its_warnings_and_opens_no_session() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().to_string_lossy().into_owned();
  std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
  std::fs::write(
    dir.path().join(".claude/toolu.config.json"),
    r#"{"version":1,"hooks":{"post-tools":false}}"#,
  )
  .unwrap();
  let env = Env::from_pairs([("HOME", home.as_str()), ("TOOLU_HOST_OVERRIDE", "nope")]);
  let opts = options(&env, dir.path());
  match Session::open(Phase::Post, &opts) {
    Opened::Disabled(warnings) => assert!(
      warnings.starts_with("toolu-host: invalid TOOLU_HOST_OVERRIDE 'nope'"),
      "{warnings}"
    ),
    Opened::Ready(..) => panic!("post-tools is off"),
  }
  match Session::open(Phase::Pre, &opts) {
    Opened::Ready(session, _) => {
      assert_eq!(
        session.env.get("TOOLU_CONFIG_DIR"),
        Some(format!("{home}/.claude").as_str())
      );
      assert_eq!(session.cwd(), dir.path());
    }
    Opened::Disabled(_) => panic!("pre-tools is on"),
  }
}
