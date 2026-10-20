use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::{Muse, argv, private_home, require_auth, require_unshadowed};
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_steps::{Driver, Run, Step};

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
  std::fs::create_dir_all(dir).unwrap();
  let file = dir.join(name);
  std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
  file
}

#[test]
fn the_command_line_is_a_never_asking_headless_exec() {
  assert_eq!(
    argv(Path::new("/bin/muse"), "go").join("|"),
    "/bin/muse|exec|--json|--approval-mode|never|--no-foreign-personal-context\
     |--disable-reminders|--disable-web-tools|go"
  );
}

#[test]
fn auth_follows_the_launchers_lookup_order() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = std::fs::canonicalize(tmp.path()).unwrap();
  let explicit = dir.join("explicit.json");
  let xdg = dir.join("xdg");
  let home = dir.join("home");
  let vars = |pairs: Vec<(&'static str, PathBuf)>| {
    move |key: &str| {
      pairs
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| OsString::from(value))
    }
  };
  let all = vars(vec![
    ("MUSE_AUTH_PATH", explicit.clone()),
    ("XDG_CONFIG_HOME", xdg.clone()),
    ("HOME", home.clone()),
  ]);
  let err = require_auth(&all).unwrap_err();
  assert_eq!(
    err,
    format!("no Muse auth at {}; run `muse login`", explicit.display())
  );
  std::fs::write(&explicit, "{}").unwrap();
  assert_eq!(require_auth(&all).unwrap(), explicit);
  std::fs::create_dir_all(xdg.join("muse")).unwrap();
  std::fs::write(xdg.join("muse/auth.json"), "{}").unwrap();
  let no_explicit = vars(vec![
    ("XDG_CONFIG_HOME", xdg.clone()),
    ("HOME", home.clone()),
  ]);
  assert_eq!(
    require_auth(&no_explicit).unwrap(),
    xdg.join("muse/auth.json")
  );
  let home_only = vars(vec![("HOME", home)]);
  assert!(
    require_auth(&home_only)
      .unwrap_err()
      .contains(".config/muse/auth.json")
  );
  assert_eq!(require_auth(&vars(vec![])).unwrap_err(), "HOME is not set");
}

#[test]
fn a_fixed_directory_toolu_that_is_not_the_binary_is_refused() {
  let dir = tempfile::tempdir().unwrap();
  let dir = std::fs::canonicalize(dir.path()).unwrap();
  let bin = script(&dir.join("checkout"), "toolu", "");
  let (first, second) = (dir.join("first"), dir.join("second"));
  std::fs::create_dir_all(&first).unwrap();
  let dirs = vec![first.clone(), second.clone()];
  assert_eq!(require_unshadowed(&dirs, &bin), Ok(()));
  std::fs::write(first.join("toolu"), "not executable").unwrap();
  assert_eq!(require_unshadowed(&dirs, &bin), Ok(()));
  let other = script(&second, "toolu", "");
  let err = require_unshadowed(&dirs, &bin).unwrap_err();
  assert_eq!(
    err,
    format!(
      "{} would answer Muse hooks instead of {}: Muse forwards no TOOLU_BIN",
      other.display(),
      bin.display()
    )
  );
  std::fs::remove_file(&other).unwrap();
  std::os::unix::fs::symlink(&bin, &other).unwrap();
  assert_eq!(require_unshadowed(&dirs, &bin), Ok(()));
}

#[test]
fn the_private_home_links_the_auth_and_the_binary() {
  let dir = tempfile::tempdir().unwrap();
  let (auth, bin) = (dir.path().join("auth.json"), dir.path().join("toolu"));
  let home = private_home(dir.path(), &auth, &bin).unwrap();
  assert_eq!(home, dir.path().join("muse-home"));
  assert_eq!(
    std::fs::read_link(home.join(".config/muse/auth.json")).unwrap(),
    auth
  );
  assert_eq!(
    std::fs::read_link(home.join(".local/bin/toolu")).unwrap(),
    bin
  );
  let mode = std::fs::metadata(&home).unwrap().permissions().mode();
  assert_eq!(mode & 0o777, 0o700);
  assert!(
    private_home(dir.path(), &auth, &bin)
      .unwrap_err()
      .contains("cannot create")
  );
}

fn env(base: &Path) -> ChildEnv {
  let path = OsString::from("/usr/bin:/bin");
  ChildEnv::new(Path::new("/x/toolu"), &path, &base.join("shims")).unwrap()
}

#[test]
fn prepare_installs_and_approves_both_plugins_in_the_private_home() {
  let dir = tempfile::tempdir().unwrap();
  let log = dir.path().join("calls.log");
  let exe = script(
    dir.path(),
    "muse",
    &format!(
      "echo \"$HOME|${{XDG_CONFIG_HOME-unset}}|$*\" >> '{}'",
      log.display()
    ),
  );
  let muse = Muse {
    exe,
    repo: PathBuf::from("/the repo"),
  };
  let home = dir.path().join("muse-home");
  let env = env(dir.path()).with_muse_home(&home);
  let run = Run {
    root: dir.path(),
    env: &env,
  };
  muse.prepare(&run, dir.path()).unwrap();
  let h = home.display();
  assert_eq!(
    std::fs::read_to_string(log).unwrap(),
    format!(
      "{h}|unset|plugins install /the repo/plugins/toolu --json\n\
       {h}|unset|plugins approve toolu --json\n\
       {h}|unset|plugins install /the repo/plugins/ts-quality --json\n\
       {h}|unset|plugins approve ts-quality --json\n"
    )
  );
}

#[test]
fn a_failed_install_stops_prepare() {
  let dir = tempfile::tempdir().unwrap();
  let exe = script(
    dir.path(),
    "muse",
    "echo 'plugin manifest is invalid' >&2; exit 1",
  );
  let muse = Muse {
    exe,
    repo: PathBuf::from("/r"),
  };
  let env = env(dir.path());
  let run = Run {
    root: dir.path(),
    env: &env,
  };
  let err = muse.prepare(&run, dir.path()).unwrap_err();
  assert!(err.contains("plugin manifest is invalid"), "{err}");
}

#[test]
fn the_muse_edit_keeps_the_failure_it_recorded_under_agents() {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir_all(project.join(".agents/tmp")).unwrap();
  let failing = "{\"status\": \"failing\"}";
  let body = format!(
    "cd '{p}'\nprintf 'a\\nb\\n' > sample.ts\nprintf '%s' '{failing}' > .agents/tmp/quality-gate-status.json\n\
     sleep 0.3\nprintf 'fixed\\n' > sample.ts\nprintf '{{}}' > .agents/tmp/quality-gate-status.json",
    p = project.display()
  );
  let muse = Muse {
    exe: script(dir.path(), "muse", &body),
    repo: PathBuf::new(),
  };
  let env = env(dir.path());
  let run = Run {
    root: dir.path(),
    env: &env,
  };
  muse.edit(&run, &project).unwrap();
  assert_eq!(
    std::fs::read_to_string(project.join("sample.ts")).unwrap(),
    "a\nb\n"
  );
  assert!(dir.path().join("muse-edit.jsonl").is_file());
  let quiet = Muse {
    exe: script(dir.path(), "muse", "echo nothing happened"),
    repo: PathBuf::new(),
  };
  let fresh = dir.path().join("fresh");
  std::fs::create_dir_all(&fresh).unwrap();
  let err = quiet.edit(&run, &fresh).unwrap_err();
  assert!(
    err.starts_with("muse did not record a live failing edit"),
    "{err}"
  );
}

#[test]
fn the_muse_driver_stages_before_its_commit_prompt() {
  let muse = Muse {
    exe: "muse".into(),
    repo: "/repo".into(),
  };
  assert_eq!(muse.name(), "muse");
  let prompts = muse.prompts();
  assert!(prompts.stages_first && !prompts.commit.contains("git add"));
  let step = Step {
    name: "x",
    tools: "Bash",
    prompt: "hi",
  };
  assert_eq!(
    Driver::argv(&muse, &step, Path::new("/p"))
      .last()
      .map(String::as_str),
    Some("hi")
  );
}
