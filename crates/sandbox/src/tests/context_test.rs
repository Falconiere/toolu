use std::path::{Path, PathBuf};

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{Context, env_of};
use crate::facts::{Facts, Os};
use crate::settings::Backend;

const MAC: Facts = Facts {
  os: Os::Macos,
  cores: 12,
  memory_gib: 32,
};

fn ctx() -> Ctx {
  Ctx {
    host: Some(Host::Claude),
    ..Ctx::default()
  }
}

/// A home with `config` as the user's toolu config, when given.
fn home(config: Option<&str>) -> (tempfile::TempDir, Env, PathBuf) {
  let dir = tempfile::tempdir().unwrap();
  let user = dir.path().join("user");
  std::fs::create_dir_all(&user).unwrap();
  if let Some(text) = config {
    std::fs::write(user.join("toolu.config.json"), text).unwrap();
  }
  let env = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("PATH", "/usr/bin:/bin"),
    ("TOOLU_USER_CONFIG_DIR", user.to_str().unwrap()),
  ]);
  let cwd = dir.path().join("work");
  std::fs::create_dir_all(&cwd).unwrap();
  (dir, env, cwd)
}

#[test]
fn without_a_config_the_machine_decides() {
  let (_dir, env, cwd) = home(None);
  let context = Context::at(&ctx(), env, &cwd, MAC).unwrap();
  assert_eq!(context.settings.backend, Backend::Lima);
  assert_eq!(context.facts, MAC);
  assert_eq!(context.project.root, None);
}

#[test]
fn the_sandbox_section_overrides_the_machine() {
  let (dir, env, cwd) = home(Some(
    r#"{"version":1,"sandbox":{"backend":"host","mounts":["~/src"]}}"#,
  ));
  let context = Context::at(&ctx(), env, &cwd, MAC).unwrap();
  assert_eq!(context.settings.backend, Backend::Host);
  assert_eq!(context.settings.mounts, [dir.path().join("src")]);
}

#[test]
fn an_invalid_config_or_setting_fails_closed_with_exit_1() {
  let (_dir, env, cwd) = home(Some(r#"{"version":1,"nope":true}"#));
  let err = Context::at(&ctx(), env, &cwd, MAC).unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert!(
    err.message.starts_with("the toolu config is invalid: "),
    "{}",
    err.message
  );
  let (_dir, env, cwd) = home(Some(r#"{"version":1,"sandbox":{"cpus":"many"}}"#));
  let err = Context::at(&ctx(), env, &cwd, MAC).unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert!(
    err.message.starts_with("sandbox.cpus must be"),
    "{}",
    err.message
  );
}

#[test]
fn a_sandbox_section_that_is_not_an_object_fails_closed_unless_it_is_null() {
  for (config, shown) in [
    (r#"{"version":1,"sandbox":"host"}"#, "\"host\""),
    (r#"{"version":1,"sandbox":["host"]}"#, "[\"host\"]"),
    (r#"{"version":1,"sandbox":true}"#, "true"),
  ] {
    let (_dir, env, cwd) = home(Some(config));
    let err = Context::at(&ctx(), env, &cwd, MAC).unwrap_err();
    assert_eq!(err.exit, Exit::Failure, "{config}");
    assert_eq!(
      err.message,
      format!("sandbox must be an object, not {shown}")
    );
  }
  let (_dir, env, cwd) = home(Some(r#"{"version":1,"sandbox":null}"#));
  let context = Context::at(&ctx(), env, &cwd, MAC).unwrap();
  assert_eq!(context.settings.backend, Backend::Lima);
}

#[test]
fn config_dir_becomes_toolu_config_dir() {
  let with = Ctx {
    config_dir: Some(Path::new("/cfg").to_path_buf()),
    ..Ctx::default()
  };
  assert_eq!(env_of(&with).get("TOOLU_CONFIG_DIR"), Some("/cfg"));
  let ours = env_of(&Ctx::default());
  assert_eq!(ours.get("HOME"), Env::process().get("HOME"));
}

#[test]
fn a_project_config_cannot_set_where_commands_run_or_what_they_reach() {
  let (dir, env, cwd) = home(Some(
    r#"{"version":1,"sandbox":{"mounts":["~/src"],"cpus":6}}"#,
  ));
  let project = dir.path().join("project");
  std::fs::create_dir_all(project.join(".claude")).unwrap();
  std::fs::write(
    project.join(".claude/toolu.config.json"),
    r#"{"version":1,"sandbox":{"backend":"host","mounts":["~/"],"template":"https://attacker/x.yaml","cpus":3,"memory":9}}"#,
  )
  .unwrap();
  let env = env.with("TOOLU_PROJECT_DIR", project.to_str().unwrap());
  let context = Context::at(&ctx(), env, &cwd, MAC).unwrap();
  // The project file is read: it still sets the size.
  assert_eq!((context.settings.cpus, context.settings.memory_gib), (3, 9));
  // But not the backend, the template or the mounts.
  assert_eq!(context.settings.backend, Backend::Lima);
  assert_eq!(context.settings.template, None);
  assert_eq!(context.settings.mounts, [dir.path().join("src")]);
}
