use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Opened, Session, post_cwd, post_env};
use crate::dispatch::{DEFAULT_MODULE_TIMEOUT, DispatchOptions, Phase};

fn options<'a>(env: &'a Env, cwd: &'a Path) -> DispatchOptions<'a> {
  DispatchOptions {
    env,
    cwd,
    lib_dir: Path::new("/lib"),
    builtins: &[],
    rules: &[],
    quality_rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    split_by_caller: false,
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

#[test]
fn cursor_roots_a_post_tool_run_at_its_project_not_the_hook_process_directory() {
  let dir = tempfile::tempdir().unwrap();
  let project = std::fs::canonicalize(dir.path()).unwrap();
  let elsewhere = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("CURSOR_PROJECT_DIR", project.to_str().unwrap())]);
  let roots = Roots::new(env, Some(Host::Cursor));
  assert_eq!(post_cwd(Host::Cursor, &roots, elsewhere.path()), project);
  assert_eq!(
    post_cwd(Host::Claude, &roots, elsewhere.path()),
    elsewhere.path()
  );
}

#[test]
fn a_cursor_session_works_in_its_project_and_every_other_host_in_the_hook_directory() {
  let project = tempfile::tempdir().unwrap();
  let elsewhere = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(project.path()).unwrap();
  let env = Env::from_pairs([
    ("HOME", elsewhere.path().to_str().unwrap()),
    ("CURSOR_PROJECT_DIR", root.to_str().unwrap()),
    ("CURSOR_VERSION", "2026.10"),
  ]);
  let opts = options(&env, elsewhere.path());
  for phase in [Phase::Pre, Phase::Post] {
    match Session::open(phase, &opts) {
      Opened::Ready(session, _) => assert_eq!(session.cwd(), root),
      Opened::Disabled(_) => panic!("the hook is on"),
    }
  }
  let claude = Env::from_pairs([("HOME", elsewhere.path().to_str().unwrap())]);
  let opts = options(&claude, elsewhere.path());
  match Session::open(Phase::Post, &opts) {
    Opened::Ready(session, _) => assert_eq!(session.cwd(), elsewhere.path()),
    Opened::Disabled(_) => panic!("the hook is on"),
  }
}
