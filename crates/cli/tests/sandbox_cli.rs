//! `toolu sandbox`, black-box, on the host backend (this machine), which runs for
//! real on Linux and macOS alike: `run` hands the terminal to the command, so its
//! stdin, stdout, stderr and exit status are toolu's; `--clone` runs in a clone of
//! the checkout; the settings fail closed; the VM verbs change nothing. The Lima
//! backend's default on macOS is checked through `status`.

use std::error::Error;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;

type Res<T> = Result<T, Box<dyn Error>>;

const HOST: &str = r#"{"version":1,"sandbox":{"backend":"host"}}"#;

/// A private home whose user config holds `config`, when given.
struct Home {
  _dir: tempfile::TempDir,
  path: PathBuf,
}

impl Home {
  fn new(config: Option<&str>) -> Res<Home> {
    let dir = tempfile::tempdir()?;
    let path = std::fs::canonicalize(dir.path())?;
    std::fs::create_dir_all(path.join("user"))?;
    if let Some(text) = config {
      std::fs::write(path.join("user/toolu.config.json"), text)?;
    }
    Ok(Home { _dir: dir, path })
  }

  /// `toolu <args>` from `cwd`, with only this home's environment and `stdin`.
  fn toolu(&self, cwd: &Path, args: &[&str], stdin: &str) -> Res<Output> {
    self.toolu_on(&std::env::var("PATH")?, cwd, args, stdin)
  }

  /// [`Home::toolu`] with `path` as `PATH`.
  fn toolu_on(&self, path: &str, cwd: &Path, args: &[&str], stdin: &str) -> Res<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toolu"));
    command
      .args(args)
      .current_dir(cwd)
      .env_clear()
      .env("HOME", &self.path)
      .env("PATH", path)
      .env("TOOLU_USER_CONFIG_DIR", self.path.join("user"))
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    // Coverage runs keep their profile path, so no `.profraw` lands in the cwd.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = command.spawn()?;
    child
      .stdin
      .take()
      .ok_or("no stdin pipe")?
      .write_all(stdin.as_bytes())?;
    Ok(child.wait_with_output()?)
  }
}

fn text(bytes: &[u8]) -> Res<String> {
  Ok(String::from_utf8(bytes.to_vec())?)
}

fn json(bytes: &[u8]) -> Res<Value> {
  Ok(serde_json::from_slice(bytes)?)
}

fn git(dir: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args(args)
    .current_dir(dir)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@t")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@t")
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("git {args:?} failed").into())
  }
}

#[test]
fn run_passes_stdin_stdout_stderr_and_the_exit_status_through() {
  let home = Home::new(Some(HOST)).unwrap();
  let script = "cat; echo err >&2; exit 7";
  let output = home
    .toolu(
      &home.path,
      &["sandbox", "run", "--", "sh", "-c", script],
      "in\n",
    )
    .unwrap();
  assert_eq!(output.status.code(), Some(7));
  assert_eq!(text(&output.stdout).unwrap(), "in\n");
  assert_eq!(text(&output.stderr).unwrap(), "err\n");
}

#[test]
fn run_starts_in_this_directory_even_with_a_space_in_it() {
  let home = Home::new(Some(HOST)).unwrap();
  let sub = home.path.join("a dir");
  std::fs::create_dir_all(&sub).unwrap();
  let output = home.toolu(&sub, &["sandbox", "run", "pwd"], "").unwrap();
  assert_eq!(output.status.code(), Some(0));
  assert_eq!(
    text(&output.stdout).unwrap(),
    format!("{}\n", sub.display())
  );
}

#[test]
fn json_on_run_or_shell_is_a_usage_error_with_one_document() {
  let home = Home::new(Some(HOST)).unwrap();
  for args in [
    &["--json", "sandbox", "run", "true"][..],
    &["--json", "sandbox", "shell"],
  ] {
    let output = home.toolu(&home.path, args, "").unwrap();
    assert_eq!(output.status.code(), Some(64), "{args:?}");
    assert_eq!(json(&output.stdout).unwrap()["error"]["code"], 64);
  }
}

#[test]
fn a_command_that_cannot_start_exits_69_and_a_bad_line_64() {
  let home = Home::new(Some(HOST)).unwrap();
  let missing = home
    .toolu(&home.path, &["sandbox", "run", "toolu-no-such-cmd"], "")
    .unwrap();
  assert_eq!(missing.status.code(), Some(69));
  assert_eq!(
    text(&missing.stderr).unwrap(),
    "sandbox: cannot start the command: toolu-no-such-cmd: not found on PATH\n"
  );
  let unconfirmed = home.toolu(&home.path, &["sandbox", "delete"], "").unwrap();
  assert_eq!(unconfirmed.status.code(), Some(64));
  assert_eq!(unconfirmed.stdout, b"");
  let bare = home.toolu(&home.path, &["sandbox", "run"], "").unwrap();
  assert_eq!(bare.status.code(), Some(64));
}

#[test]
fn run_clone_runs_in_a_clone_with_the_changes_and_needs_git() {
  let home = Home::new(Some(HOST)).unwrap();
  let repo = home.path.join("repo");
  std::fs::create_dir_all(repo.join("sub")).unwrap();
  git(&repo, &["init", "-q"]).unwrap();
  std::fs::write(repo.join("file.txt"), "one\n").unwrap();
  git(&repo, &["add", "-A"]).unwrap();
  git(&repo, &["commit", "-q", "-m", "init"]).unwrap();
  std::fs::write(repo.join("file.txt"), "two\n").unwrap();
  std::fs::write(repo.join("sub/new.txt"), "new\n").unwrap();
  let script = "pwd; cat ../file.txt new.txt";
  let args = ["sandbox", "run", "--clone", "--", "sh", "-c", script];
  let output = home.toolu(&repo.join("sub"), &args, "").unwrap();
  let out = text(&output.stdout).unwrap();
  assert_eq!(output.status.code(), Some(0), "{out}");
  let work = home.path.join(".cache/toolu-sandbox/work");
  assert!(out.starts_with(&work.display().to_string()), "{out}");
  assert!(out.ends_with("/sub\ntwo\nnew\n"), "{out}");
  let kept = std::fs::read_to_string(repo.join("file.txt")).unwrap();
  assert_eq!(kept, "two\n");
  let outside = home
    .toolu(&home.path, &["sandbox", "run", "--clone", "true"], "")
    .unwrap();
  assert_eq!(outside.status.code(), Some(64));
  let why = text(&outside.stderr).unwrap();
  assert!(
    why.starts_with("sandbox: --clone needs a git checkout; "),
    "{why}"
  );
}

#[test]
fn an_unknown_or_invalid_setting_fails_closed_with_exit_1() {
  for (config, message) in [
    (
      r#"{"version":1,"sandbox":{"backend":"host","cpu":2}}"#,
      "sandbox: sandbox.cpu is not a setting; ",
    ),
    (
      r#"{"version":1,"sandbox":{"backend":"docker"}}"#,
      "sandbox: sandbox.backend must be \"lima\" or \"host\", not \"docker\"\n",
    ),
  ] {
    let home = Home::new(Some(config)).unwrap();
    let output = home
      .toolu(&home.path, &["sandbox", "run", "true"], "")
      .unwrap();
    assert_eq!(output.status.code(), Some(1), "{config}");
    let why = text(&output.stderr).unwrap();
    assert!(why.starts_with(message), "{why}");
  }
}

#[test]
fn the_vm_verbs_change_nothing_on_the_host() {
  let home = Home::new(Some(HOST)).unwrap();
  for verb in [
    &["sandbox", "start"][..],
    &["sandbox", "stop"],
    &["sandbox", "delete", "--yes"],
  ] {
    let output = home.toolu(&home.path, verb, "").unwrap();
    assert_eq!(output.status.code(), Some(0), "{verb:?}");
    let said = text(&output.stdout).unwrap();
    assert!(
      said.starts_with("sandbox: the host backend has no VM to "),
      "{said}"
    );
  }
}

#[test]
fn setup_names_what_is_missing_and_the_agent_clis() {
  let home = Home::new(Some(HOST)).unwrap();
  let output = home
    .toolu(&home.path, &["--json", "sandbox", "setup"], "")
    .unwrap();
  let doc = json(&output.stdout).unwrap();
  let missing = doc["missing"].as_array().unwrap();
  let expected = if missing.is_empty() { 0 } else { 69 };
  assert_eq!(output.status.code(), Some(expected), "{doc}");
  assert_eq!(doc["next"].as_array().map(Vec::len), Some(4));
}

#[test]
fn the_default_backend_is_lima_on_macos_and_this_machine_elsewhere() {
  let home = Home::new(None).unwrap();
  let output = home
    .toolu(&home.path, &["--json", "sandbox", "status"], "")
    .unwrap();
  assert_eq!(output.status.code(), Some(0));
  let doc = json(&output.stdout).unwrap();
  let state = doc["state"].as_str().unwrap();
  if std::env::consts::OS == "macos" {
    // Without Lima on PATH it is unavailable; with it, this fresh home has no VM.
    assert_eq!(doc["backend"], "lima");
    assert!(["unavailable", "absent"].contains(&state), "{doc}");
  } else {
    assert_eq!((doc["backend"].as_str(), state), (Some("host"), "host"));
  }
}

/// A `limactl` in `<home>/bin` reporting `list` and running `shell` commands on
/// this machine, from their `--workdir`, as the VM would.
fn stub_lima(home: &Path, list: &str) -> Res<String> {
  use std::os::unix::fs::PermissionsExt as _;
  let bin = home.join("bin");
  std::fs::create_dir_all(&bin)?;
  std::fs::write(bin.join("list.json"), list)?;
  let script = "#!/bin/sh\nd=$(dirname \"$0\")\ncase \"$1\" in\n  --version) echo 'limactl version 2.2.1' ;;\n  list) cat \"$d/list.json\" ;;\n  shell) cd \"$3\" && shift 5 && exec \"$@\" ;;\nesac\n";
  let program = bin.join("limactl");
  if !program.exists() {
    std::fs::write(&program, script)?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
    first_run(&program)?;
  }
  Ok(format!("{}:{}", bin.display(), std::env::var("PATH")?))
}

/// Run a script just written until the OS stops answering "text file busy": a
/// thread that forked while the file was open for writing holds a copy of the
/// descriptor until its child execs, and the file is never written again after.
fn first_run(program: &Path) -> Res<()> {
  let mut waited = 0;
  while let Err(err) = Command::new(program).arg("--version").output() {
    if err.kind() != std::io::ErrorKind::ExecutableFileBusy || waited >= 500 {
      return Err(err.into());
    }
    waited += 1;
    std::thread::sleep(std::time::Duration::from_millis(10));
  }
  Ok(())
}

#[test]
fn on_lima_run_execs_limactl_shell_with_the_guest_defaults() {
  let home = Home::new(Some(r#"{"version":1,"sandbox":{"backend":"lima"}}"#)).unwrap();
  let repo = home.path.join("repo");
  std::fs::create_dir_all(&repo).unwrap();
  let state = home.path.join(".local/state/toolu/sandbox");
  std::fs::create_dir_all(&state).unwrap();
  std::fs::write(state.join("toolu-sandbox.json"), "{}").unwrap();
  let vm = |status: &str, mount: &Path| {
    let mounts = serde_json::json!([{ "location": mount, "writable": true }]);
    format!(
      "{}\n",
      serde_json::json!({ "name": "toolu-sandbox", "status": status, "config": { "mounts": mounts } })
    )
  };
  let path = stub_lima(&home.path, &vm("Running", &repo)).unwrap();
  let script = "pwd; echo \"$TMPDIR\"; exit 9";
  let args = ["sandbox", "run", "--", "sh", "-c", script];
  let output = home.toolu_on(&path, &repo, &args, "").unwrap();
  assert_eq!(
    output.status.code(),
    Some(9),
    "{}",
    text(&output.stderr).unwrap()
  );
  let expected = format!("{}\n/var/cache/toolu-sandbox/tmp\n", repo.display());
  assert_eq!(text(&output.stdout).unwrap(), expected);
  stub_lima(&home.path, &vm("Running", Path::new("/elsewhere"))).unwrap();
  let refused = home.toolu_on(&path, &repo, &args, "").unwrap();
  assert_eq!(refused.status.code(), Some(69));
  let why = text(&refused.stderr).unwrap();
  assert!(
    why.starts_with("sandbox: the running VM does not mount "),
    "{why}"
  );
  stub_lima(&home.path, "").unwrap();
  let none = home.toolu_on(&path, &repo, &args, "").unwrap();
  assert_eq!(
    text(&none.stderr).unwrap(),
    "sandbox: there is no VM yet; run `toolu sandbox setup`\n"
  );
}
