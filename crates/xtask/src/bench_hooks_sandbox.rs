//! The fixed sandbox every hook entry runs in for `cargo xtask bench-hooks`: a
//! git project with a README and a TypeScript file, toolu (and, for the full
//! fixture, the four registry plugins) marked installed in a private home, and
//! their `register` hooks run once, so every run loads the same registry.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::env_scrub::{inherited, scrub};
use crate::scratch;

/// The plugins whose `SessionStart` register hook writes the registry.
const REGISTRY_PLUGINS: [&str; 4] = ["ast-grep", "ts-quality", "python-quality", "rust-quality"];

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
      sandbox.register(repo, plugin)?;
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

  fn register(&self, repo: &Path, plugin: &str) -> Result<(), String> {
    let root = repo.join("plugins").join(plugin);
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
      .arg(&root)
      .current_dir(&self.project)
      .env_clear()
      .envs(self.env(&root))
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
    if output.status.success() {
      return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
      "bench sandbox: register {plugin} exited {:?}: {}",
      output.status.code(),
      stderr.trim()
    ))
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
}

#[cfg(test)]
#[path = "tests/bench_hooks_sandbox_test.rs"]
mod tests;
