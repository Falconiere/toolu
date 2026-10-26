//! The fixed sandbox every hook entry runs in for `cargo xtask bench-hooks`: a
//! git project with a README and a TypeScript file, toolu (and, for the full
//! fixture, the four registry plugins) marked installed in a private home, and
//! their `register` hooks run once per host (Claude Code, then Muse), so every
//! run loads the same registry.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::env_scrub::{inherited, scrub};
use crate::scratch;

/// The plugins whose `SessionStart` register hook writes the registry.
const REGISTRY_PLUGINS: [&str; 4] = ["ast-grep", "ts-quality", "python-quality", "rust-quality"];

/// What Muse passes a hook from its own environment (probed on Muse 1.4.11);
/// `HOME` and `PWD` are the sandbox's own.
const MUSE_FORWARDED: [&str; 6] = ["LANG", "LOGNAME", "PATH", "SHELL", "TERM", "USER"];

/// The session id a Muse hook sees.
const MUSE_SESSION: &str = "hook-bench";

/// A temporary `project` and `home`, removed on drop.
pub(crate) struct Sandbox {
  root: PathBuf,
  pub(crate) project: PathBuf,
  home: PathBuf,
  bin: PathBuf,
}

impl Drop for Sandbox {
  fn drop(&mut self) {
    let _gone = std::fs::remove_dir_all(&self.root);
  }
}

fn run(command: &mut Command, what: &str) -> Result<(), String> {
  let output = command
    .output()
    .map_err(|err| format!("bench sandbox: cannot run {what}: {err}"))?;
  if output.status.success() {
    return Ok(());
  }
  let stderr = String::from_utf8_lossy(&output.stderr);
  Err(format!("bench sandbox: {what} failed: {}", stderr.trim()))
}

/// Whether `plugin`'s `register` run succeeded. A failed registration is a
/// `systemMessage` with exit 0, so it never fails the session (#642).
fn registered(plugin: &str, output: &Output) -> Result<(), String> {
  let stdout = String::from_utf8_lossy(&output.stdout);
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!(
      "bench sandbox: register {plugin} exited {:?}: {}",
      output.status.code(),
      stderr.trim()
    ));
  }
  if stdout.contains("registration failed") {
    return Err(format!(
      "bench sandbox: register {plugin}: {}",
      stdout.trim()
    ));
  }
  Ok(())
}

fn git(project: &Path, args: &[&str]) -> Result<(), String> {
  let mut command = Command::new("git");
  command.arg("-C").arg(project).args(args);
  scrub(&mut command);
  run(&mut command, &format!("git {}", args.join(" ")))
}

impl Sandbox {
  /// The private directory that holds the project, the home and the measure reports.
  pub(crate) fn scratch(&self) -> &Path {
    &self.root
  }

  /// The sandbox for `repo`'s plugins, run with the native `bin`.
  pub(crate) fn new(repo: &Path, bin: &Path, registry_plugins: bool) -> Result<Sandbox, String> {
    let root = scratch::create("toolu-hook-bench")?;
    let sandbox = Sandbox {
      project: root.join("project"),
      home: root.join("home"),
      bin: bin.to_path_buf(),
      root,
    };
    let io = |err: std::io::Error| format!("bench sandbox: {err}");
    std::fs::create_dir_all(sandbox.project.join("src")).map_err(io)?;
    std::fs::write(sandbox.project.join("README.md"), "# bench\n\nhello\n").map_err(io)?;
    std::fs::write(
      sandbox.project.join("src/app.ts"),
      "export const answer = 42;\n",
    )
    .map_err(io)?;
    sandbox.init_git()?;
    let plugins = if registry_plugins {
      REGISTRY_PLUGINS.as_slice()
    } else {
      &[]
    };
    sandbox.install(plugins)?;
    for plugin in plugins {
      let root = repo.join("plugins").join(plugin);
      sandbox.register(plugin, sandbox.env(&root), &root)?;
      sandbox.register(plugin, sandbox.muse_env(&root, plugin), &root)?;
    }
    Ok(sandbox)
  }

  fn init_git(&self) -> Result<(), String> {
    let project = &self.project;
    git(project, &["init", "-q", "-b", "main"])?;
    for (key, value) in [
      ("user.email", "bench@toolu.test"),
      ("user.name", "toolu bench"),
      ("commit.gpgsign", "false"),
      ("maintenance.auto", "false"),
      ("gc.auto", "0"),
    ] {
      git(project, &["config", key, value])?;
    }
    git(project, &["add", "-A"])?;
    git(project, &["commit", "-q", "-m", "bench: initial commit"])
  }

  /// Mark toolu and `plugins` installed where Claude Code records them.
  fn install(&self, plugins: &[&str]) -> Result<(), String> {
    let specs = std::iter::once("toolu").chain(plugins.iter().copied());
    let map: serde_json::Map<String, serde_json::Value> = specs
      .map(|plugin| {
        (
          format!("{plugin}@toolu"),
          serde_json::json!([{"scope": "user"}]),
        )
      })
      .collect();
    let file = self.home.join(".claude/plugins/installed_plugins.json");
    let dir = file.parent().ok_or("bench sandbox: no plugins dir")?;
    std::fs::create_dir_all(dir).map_err(|err| format!("bench sandbox: {err}"))?;
    let doc = serde_json::json!({"version": 2, "plugins": map});
    std::fs::write(&file, format!("{doc}\n")).map_err(|err| format!("bench sandbox: {err}"))
  }

  fn register(
    &self,
    plugin: &str,
    env: Vec<(OsString, OsString)>,
    root: &Path,
  ) -> Result<(), String> {
    let mut command = Command::new(&self.bin);
    command
      .args([
        plugin,
        "hook",
        "register",
        "--event",
        "SessionStart",
        "--plugin-root",
      ])
      .arg(root)
      .current_dir(&self.project)
      .env_clear()
      .envs(env)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    let mut child = command
      .spawn()
      .map_err(|err| format!("bench sandbox: register {plugin}: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
      stdin
        .write_all(b"{}")
        .map_err(|err| format!("bench sandbox: {err}"))?;
    }
    let output = child
      .wait_with_output()
      .map_err(|err| format!("bench sandbox: {err}"))?;
    registered(plugin, &output)
  }

  /// The hook environment: this process's minus host-session keys, then the
  /// sandbox home and Claude's roots for `plugin_root`. The hook is spawned by path,
  /// so `TOOLU_BIN` stays unset, as it was when the budgets were measured.
  pub(crate) fn env(&self, plugin_root: &Path) -> Vec<(OsString, OsString)> {
    let mut env = inherited(&["PLUGIN_ROOT"]);
    let text = |key: &str, path: &Path| (OsString::from(key), path.as_os_str().to_owned());
    env.extend([
      text("HOME", &self.home),
      text("CLAUDE_PLUGIN_ROOT", plugin_root),
      text("CLAUDE_PROJECT_DIR", &self.project),
      text("CLAUDE_CONFIG_DIR", &self.home.join(".claude")),
    ]);
    env
  }

  /// A Muse hook's environment: only the keys Muse forwards, its plugin and
  /// session variables with their Claude Code and Codex aliases, and `TOOLU_BIN`.
  /// Muse never forwards `TOOLU_BIN`; the bench sets it so the resolver measures
  /// `--bin`, not a `toolu` installed on the machine.
  pub(crate) fn muse_env(&self, plugin_root: &Path, plugin: &str) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = MUSE_FORWARDED
      .iter()
      .filter_map(|key| std::env::var_os(key).map(|value| (OsString::from(key), value)))
      .collect();
    let data = self
      .home
      .join(".local/share/muse/plugins/data")
      .join(plugin);
    let path = |key: &str, path: &Path| (OsString::from(key), path.as_os_str().to_owned());
    let word = |key: &str, value: &str| (OsString::from(key), OsString::from(value));
    env.extend([
      path("HOME", &self.home),
      path("PWD", &self.project),
      word("MUSE_PLUGIN_ID", plugin),
      path("MUSE_PLUGIN_ROOT", plugin_root),
      path("MUSE_PLUGIN_DATA_DIR", &data),
      word("MUSE_SESSION_ID", MUSE_SESSION),
      word("MUSE_THREAD_ID", MUSE_SESSION),
      path("CLAUDE_PLUGIN_ROOT", plugin_root),
      path("CLAUDE_PLUGIN_DATA", &data),
      path("PLUGIN_ROOT", plugin_root),
      path("PLUGIN_DATA", &data),
      path("TOOLU_BIN", &self.bin),
    ]);
    env
  }
}

#[cfg(test)]
#[path = "tests/bench_hooks_sandbox_test.rs"]
mod tests;
