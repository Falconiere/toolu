//! `toolu hook pre-tools` and `toolu hook post-tools` (AC-13, AC-5): the real
//! binary over a real registry, its streams and exit status byte for byte.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Output;

/// The `toolu` binary under test.
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A helper's result; the tests unwrap it.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// An executable `path` running `body` under `shell`.
fn executable(path: &Path, shell: &str, body: &str) -> Res<()> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(path, format!("#!{shell}\n{body}\n"))?;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

/// A sandbox: `home` (Claude's config root is `home/.claude`) and `project`.
struct Sandbox {
  _dir: tempfile::TempDir,
  root: PathBuf,
}

impl Sandbox {
  fn new() -> Res<Sandbox> {
    let dir = tempfile::tempdir()?;
    let root = std::fs::canonicalize(dir.path())?;
    std::fs::create_dir_all(root.join("project"))?;
    Ok(Sandbox { _dir: dir, root })
  }

  fn registry(&self, dir: &str) -> PathBuf {
    self.root.join("home/.claude/toolu").join(dir)
  }

  fn module(&self, dir: &str, file: &str, body: &str) -> Res<()> {
    executable(&self.registry(dir).join(file), "/usr/bin/env bash", body)
  }

  /// `toolu hook <name>` with `stdin`, `path` first on `PATH`.
  fn hook(&self, name: &str, stdin: impl Into<Vec<u8>>, path: Option<&Path>) -> Res<Output> {
    self.hook_args(name, stdin, path, &[])
  }

  /// [`Sandbox::hook`] with `extra` flags after `--event`.
  fn hook_args(
    &self,
    name: &str,
    stdin: impl Into<Vec<u8>>,
    path: Option<&Path>,
    extra: &[&str],
  ) -> Res<Output> {
    let system = std::env::var("PATH")?;
    let path = path.map_or(system.clone(), |first| {
      format!("{}:{system}", first.display())
    });
    let event = if name == "pre-tools" {
      "PreToolUse"
    } else {
      "PostToolUse"
    };
    Ok(
      assert_cmd::Command::new(TOOLU)
        .args(["hook", name, "--event", event])
        .args(extra)
        .env_clear()
        .env("PATH", path)
        .env("HOME", self.root.join("home"))
        .env("CLAUDE_PROJECT_DIR", self.root.join("project"))
        .current_dir(self.root.join("project"))
        .write_stdin(stdin.into())
        .output()?,
    )
  }
}

fn streams(output: &Output) -> (String, String, Option<i32>) {
  (
    String::from_utf8_lossy(&output.stdout).into_owned(),
    String::from_utf8_lossy(&output.stderr).into_owned(),
    output.status.code(),
  )
}

const BASH: &str = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;
const READ: &str =
  r#"{"tool_name":"Read","tool_input":{"file_path":"/etc/hosts"},"session_id":"s1"}"#;

#[test]
fn a_module_exiting_2_blocks_the_tool_with_its_stderr() {
  let sb = Sandbox::new().unwrap();
  sb.module(
    "pre-tools.d",
    "x@t__deny.sh",
    "printf 'denied by module\\n' >&2\nexit 2",
  )
  .unwrap();
  let out = sb.hook("pre-tools", BASH, None).unwrap();
  assert_eq!(
    streams(&out),
    (String::new(), "denied by module\n".to_owned(), Some(2))
  );
  sb.module(
    "pre-tools.d",
    "x@t__deny.sh",
    "printf 'no newline' >&2\nexit 2",
  )
  .unwrap();
  let out = sb.hook("pre-tools", BASH, None).unwrap();
  assert_eq!(streams(&out).1, "no newline\n", "the CLI ends the line");
  sb.module("pre-tools.d", "x@t__deny.sh", "printf '\\n' >&2\nexit 2")
    .unwrap();
  assert_eq!(
    streams(&sb.hook("pre-tools", BASH, None).unwrap()),
    (String::new(), String::new(), Some(2))
  );
}

#[test]
fn a_post_tool_block_is_printed_as_written() {
  let sb = Sandbox::new().unwrap();
  let block = r#"{"decision":"block","reason":"bad edit"}"#;
  sb.module(
    "post-tools.d",
    "x@t__block.sh",
    &format!("printf '%s\\n' '{block}'"),
  )
  .unwrap();
  let edit = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}}"#;
  let out = sb.hook("post-tools", edit, None).unwrap();
  assert_eq!(
    streams(&out),
    (format!("{block}\n"), String::new(), Some(0))
  );
}

#[test]
fn a_read_with_nothing_that_fits_prints_nothing_and_spawns_nothing() {
  let sb = Sandbox::new().unwrap();
  assert_eq!(
    streams(&sb.hook("pre-tools", READ, None).unwrap()),
    (String::new(), String::new(), Some(0))
  );
  let manifest =
    r#"{"version":1,"spec":"x@t","name":"r","event":"tool/pre","matcher":"Write|Edit"}"#;
  std::fs::create_dir_all(sb.registry("pre-tools.d")).unwrap();
  std::fs::write(sb.registry("pre-tools.d").join("x@t__r.json"), manifest).unwrap();
  let log = sb.root.join("spawned");
  for name in ["bash", "bun"] {
    let body = format!("echo {name} >> {}\nexit 1", log.display());
    executable(&sb.root.join("bin").join(name), "/bin/sh", &body).unwrap();
  }
  let out = sb
    .hook("pre-tools", READ, Some(&sb.root.join("bin")))
    .unwrap();
  assert_eq!(streams(&out), (String::new(), String::new(), Some(0)));
  assert!(!log.exists(), "no module process may be spawned");
  sb.module("pre-tools.d", "y@t__probe.sh", "exit 0").unwrap();
  std::fs::write(
    sb.registry("pre-tools.d").join("z@t__probe.js"),
    "export default 42;",
  )
  .unwrap();
  let out = sb
    .hook("pre-tools", READ, Some(&sb.root.join("bin")))
    .unwrap();
  let failed = "toolu-registry: module z@t__probe.js failed: bridge exited 1; output skipped\n\
                toolu-dispatch: module y@t__probe.sh exited 1; output skipped\n";
  assert_eq!(streams(&out), (String::new(), failed.to_owned(), Some(0)));
  let spawned = std::fs::read_to_string(&log).unwrap();
  assert_eq!(
    spawned, "bash\nbun\n",
    "control: the sentinels see a .sh and a .js module run"
  );
}

#[test]
fn an_unreadable_payload_blocks_and_a_disabled_hook_is_silent() {
  let sb = Sandbox::new().unwrap();
  let (stdout, stderr, code) =
    streams(&sb.hook("pre-tools", vec![0xff, 0xfe, b'{'], None).unwrap());
  assert_eq!((stdout.as_str(), code), ("", Some(2)));
  assert_eq!(
    stderr,
    "blocked: toolu PreToolUse hook failed: the hook payload could not be read: stream did not contain valid UTF-8\n"
  );
  sb.module("pre-tools.d", "x@t__deny.sh", "exit 2").unwrap();
  let config = sb.root.join("project/.claude/toolu.config.json");
  std::fs::create_dir_all(config.parent().unwrap()).unwrap();
  std::fs::write(&config, r#"{"version":1,"hooks":{"pre-tools":false}}"#).unwrap();
  assert_eq!(
    streams(&sb.hook("pre-tools", BASH, None).unwrap()),
    (String::new(), String::new(), Some(0))
  );
}

#[test]
fn the_plugin_root_gives_modules_their_lib_dir() {
  let sb = Sandbox::new().unwrap();
  let root = sb.root.join("plugin");
  let manifest = format!(
    r#"{{"name":"toolu","version":"{}","hookProtocol":{}}}"#,
    env!("CARGO_PKG_VERSION"),
    toolu_protocol::HOOK_PROTOCOL
  );
  std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
  std::fs::write(root.join(".claude-plugin/plugin.json"), manifest).unwrap();
  sb.module(
    "pre-tools.d",
    "x@t__lib.sh",
    "printf '%s\\n' \"$TOOLU_LIB_DIR\" >&2\nexit 2",
  )
  .unwrap();
  let flag = root.to_str().unwrap();
  let out = sb
    .hook_args("pre-tools", BASH, None, &["--plugin-root", flag])
    .unwrap();
  let lib = format!("{flag}/hooks/lib\n");
  assert_eq!(streams(&out), (String::new(), lib, Some(2)));
}
