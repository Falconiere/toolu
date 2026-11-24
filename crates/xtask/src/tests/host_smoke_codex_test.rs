use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::{Codex, argv, private_home};
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_steps::{Driver, Run, Step};

fn env(base: &Path) -> ChildEnv {
  let path = OsString::from("/usr/bin:/bin");
  ChildEnv::new(Path::new("/x/toolu"), &path, &base.join("shims")).unwrap()
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
  let file = dir.join(name);
  std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
  file
}

#[test]
fn the_command_line_is_a_never_asking_sandboxed_exec() {
  let words = argv(Path::new("/bin/codex"), Path::new("/p q"), "go");
  assert_eq!(
    words.join("|"),
    "/bin/codex|--ask-for-approval|never|--sandbox|workspace-write|--dangerously-bypass-hook-trust\
     |exec|--json|--ephemeral|-C|/p q|go"
  );
}

#[test]
fn the_private_home_links_the_users_auth() {
  let dir = tempfile::tempdir().unwrap();
  let auth = dir.path().join("auth.json");
  assert!(
    private_home(dir.path(), &auth)
      .unwrap_err()
      .contains("no Codex auth")
  );
  std::fs::write(&auth, "{}").unwrap();
  let home = private_home(dir.path(), &auth).unwrap();
  assert_eq!(std::fs::read_link(home.join("auth.json")).unwrap(), auth);
  let mode = std::fs::metadata(&home).unwrap().permissions().mode();
  assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn prepare_adds_the_marketplace_and_both_plugins() {
  let dir = tempfile::tempdir().unwrap();
  let log = dir.path().join("calls.log");
  let exe = script(
    dir.path(),
    "codex",
    &format!("echo \"$@\" >> '{}'", log.display()),
  );
  let codex = Codex {
    exe,
    repo: PathBuf::from("/the repo"),
  };
  let run = Run {
    root: dir.path(),
    env: &env(dir.path()),
  };
  codex.prepare(&run, dir.path()).unwrap();
  let calls = std::fs::read_to_string(log).unwrap();
  assert_eq!(
    calls,
    "plugin marketplace add /the repo\nplugin add toolu@toolu\nplugin add ts-quality@toolu\n"
  );
}

#[test]
fn the_codex_driver_names_itself_and_keeps_its_prompts() {
  let codex = Codex {
    exe: "codex".into(),
    repo: "/repo".into(),
  };
  let prompts = codex.prompts();
  assert_eq!(codex.name(), "codex");
  assert!(!prompts.commit.contains("git add"));
  let step = Step {
    name: "x",
    tools: "Bash",
    prompt: "hi",
  };
  assert_eq!(
    Driver::argv(&codex, &step, Path::new("/p"))
      .last()
      .map(String::as_str),
    Some("hi")
  );
}
