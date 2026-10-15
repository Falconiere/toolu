use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::{Codex, argv, observe, private_home};
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

const FAILING: &str = "{\"status\": \"failing\"}";

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
fn observe_waits_for_a_failing_gate_file() {
  let dir = tempfile::tempdir().unwrap();
  let (gate, source) = (dir.path().join("gate.json"), dir.path().join("sample.ts"));
  std::fs::write(&source, "edited\n").unwrap();
  assert!(observe(&gate, &source).is_none());
  std::fs::write(&gate, "{\"status\": \"passing\"}").unwrap();
  assert!(observe(&gate, &source).is_none());
  std::fs::write(&gate, FAILING).unwrap();
  assert_eq!(
    observe(&gate, &source),
    Some((FAILING.to_owned(), "edited\n".to_owned()))
  );
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
fn an_edit_repaired_in_the_same_turn_keeps_its_observed_failure() {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir_all(project.join(".codex/tmp")).unwrap();
  std::fs::write(project.join("sample.ts"), "export const n = 1;\n").unwrap();
  let body = format!(
    "cd '{p}'\nprintf 'a\\nb\\n' > sample.ts\nprintf '%s' '{FAILING}' > .codex/tmp/quality-gate-status.json\n\
     sleep 0.3\nprintf 'fixed\\n' > sample.ts\nprintf '{{}}' > .codex/tmp/quality-gate-status.json",
    p = project.display()
  );
  let exe = script(dir.path(), "codex", &body);
  let codex = Codex {
    exe,
    repo: PathBuf::new(),
  };
  let run = Run {
    root: dir.path(),
    env: &env(dir.path()),
  };
  codex.edit(&run, &project).unwrap();
  assert_eq!(
    std::fs::read_to_string(project.join("sample.ts")).unwrap(),
    "a\nb\n"
  );
  let gate = std::fs::read_to_string(project.join(".codex/tmp/quality-gate-status.json")).unwrap();
  assert_eq!(gate, FAILING);
  assert!(dir.path().join("codex-edit.jsonl").is_file());
}

#[test]
fn an_edit_that_never_fails_the_gate_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let exe = script(dir.path(), "codex", "echo nothing happened");
  let codex = Codex {
    exe,
    repo: PathBuf::new(),
  };
  let run = Run {
    root: dir.path(),
    env: &env(dir.path()),
  };
  let err = codex.edit(&run, dir.path()).unwrap_err();
  assert!(err.contains("did not record a live failing edit") && err.contains("nothing happened"));
}

#[test]
fn the_codex_driver_stages_before_its_commit_prompt() {
  let codex = Codex {
    exe: "codex".into(),
    repo: "/repo".into(),
  };
  let prompts = codex.prompts();
  assert_eq!(codex.name(), "codex");
  assert!(prompts.stages_first && !prompts.commit.contains("git add"));
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
