//! Putting a fresh sandbox in a module case's state: settings, config, setup, the
//! working directory, the hook's stdin and its environment.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::golden_gates_model::{ModuleCase, Res, Scope};
use crate::golden_gates_run::{EnvPatch, lossy, plugin_root, pretool_env};
use crate::golden_gates_sandbox::{Sandbox, write_at};
use crate::golden_gates_stdin::{bash_fixture, render};

/// The module files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Suite {
  /// Protected files, MCP blocker and code-edit rules.
  A,
  /// Bash commands, commit gate and quality gate.
  B,
  /// Push review, plan ledger, docs sync and agent tier.
  C,
}

/// The cwd, environment and stdin of one call.
pub(crate) struct Prepared {
  /// The working directory.
  pub(crate) cwd: PathBuf,
  /// The environment.
  pub(crate) env: EnvPatch,
  /// Standard input.
  pub(crate) stdin: String,
}

const SHIPPED_A: [&str; 3] = [
  "protected-files.txt",
  "mcp-blocklist.txt",
  "code-edit-rules.json",
];
const SHIPPED_B: [&str; 3] = [
  "bash-allowlist.txt",
  "bash-denylist.txt",
  "commit-prefixes.txt",
];

/// Every tool the hooks and the sandbox reach for.
const TOOLS: [&str; 46] = [
  "bash", "sh", "jq", "git", "awk", "grep", "sed", "tr", "cat", "head", "tail", "dirname",
  "basename", "mktemp", "rm", "mkdir", "env", "uname", "cut", "sort", "uniq", "wc", "date",
  "printf", "ls", "find", "xargs", "readlink", "realpath", "stat", "touch", "mv", "cp", "ln",
  "tee", "sleep", "id", "od", "shasum", "chmod", "cmp", "diff", "comm", "paste", "expr", "python3",
];

fn executable(path: &Path) -> bool {
  use std::os::unix::fs::PermissionsExt;
  fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// A `PATH` of symlinks to every tool in `TOOLS` except `without`.
fn path_without(sb: &Sandbox, without: &[String]) -> Res<String> {
  let bin = sb.root.join("bin-path");
  fs::create_dir_all(&bin)?;
  let search = std::env::var_os("PATH").unwrap_or_default();
  for tool in TOOLS
    .iter()
    .filter(|tool| !without.iter().any(|w| w == *tool))
  {
    let found = std::env::split_paths(&search)
      .map(|dir| dir.join(tool))
      .find(|path| executable(path));
    if let Some(found) = found {
      symlink(found, bin.join(tool))?;
    }
  }
  Ok(bin.to_string_lossy().into_owned())
}

fn write_settings(sb: &Sandbox, c: &ModuleCase, shipped: &[&str]) -> Res<PathBuf> {
  let dir = sb.root.join("settings");
  fs::create_dir_all(&dir)?;
  if c.shipped_settings {
    for file in shipped {
      fs::copy(plugin_root().join("settings").join(file), dir.join(file))?;
    }
  }
  for (file, body) in c.settings.iter().flatten() {
    write_at(&dir.join(file), body)?;
  }
  Ok(dir)
}

/// A config body as the case wrote it: its text, or the document serialized.
fn config_text(body: &Value) -> Res<String> {
  if let Value::String(text) = body {
    return Ok(text.clone());
  }
  Ok(serde_json::to_string(body)?)
}

#[derive(Deserialize)]
struct ScopedConfig {
  scope: Scope,
  body: Value,
}

fn stdin_of(sb: &Sandbox, c: &ModuleCase, suite: Suite, cwd: &Path) -> Res<String> {
  if let Some(raw) = &c.stdin {
    return Ok(raw.clone());
  }
  let fixture = match (&c.fixture, suite) {
    (Some(fixture), _) => sb.materialize(fixture, c.host)?,
    (None, Suite::A) => return Ok(String::new()),
    (None, _) => bash_fixture(c.command.as_deref().unwrap_or_default()),
  };
  Ok(render(c.host, &fixture, cwd)?.to_string())
}

fn settings_env(dir: &Path) -> EnvPatch {
  EnvPatch::from([("TOOLU_SETTINGS_DIR".to_owned(), Some(lossy(dir)))])
}

/// A case of the A file.
pub(crate) fn prepare_a(sb: &Sandbox, c: &ModuleCase) -> Res<Prepared> {
  let settings = write_settings(sb, c, &SHIPPED_A)?;
  for file in c.files.iter().flatten() {
    write_at(&sb.resolve(file, c.host)?, "x\n")?;
  }
  if let Some(config) = &c.config {
    let scoped: ScopedConfig = serde_json::from_value(config.clone())?;
    let dir = sb.config_dir(c.host, scoped.scope);
    write_at(&dir.join("toolu.config.json"), &config_text(&scoped.body)?)?;
  }
  let stdin = stdin_of(sb, c, Suite::A, &sb.project)?;
  let env = pretool_env(sb, c.host, settings_env(&settings));
  Ok(Prepared {
    cwd: sb.project.clone(),
    env,
    stdin,
  })
}

/// A case of the B or C file.
pub(crate) fn prepare_gate(sb: &Sandbox, c: &ModuleCase, suite: Suite) -> Res<Prepared> {
  let settings = write_settings(sb, c, &SHIPPED_B)?;
  if let Some(config) = &c.config {
    let path = sb
      .config_dir(c.host, Scope::Project)
      .join("toolu.config.json");
    write_at(&path, &config_text(config)?)?;
  }
  if let Some(gate) = &c.gate {
    let status = sb
      .project
      .join(c.host.state_dir())
      .join("tmp/quality-gate-status.json");
    write_at(&status, gate)?;
  }
  sb.apply(c.setup.as_deref().unwrap_or_default(), c.host)?;
  let cwd = match &c.cwd {
    Some(tag) => sb.resolve(&tag.path, c.host)?,
    None => sb.project.clone(),
  };
  let stdin = stdin_of(sb, c, suite, &cwd)?;
  let mut extra = settings_env(&settings);
  let missing = if c.no_python {
    Some(vec!["python3".to_owned()])
  } else {
    c.without.clone()
  };
  if let Some(without) = missing {
    extra.insert("PATH".to_owned(), Some(path_without(sb, &without)?));
  }
  for (key, value) in c.env.iter().flatten() {
    extra.insert(key.clone(), Some(sb.materialize_str(value, c.host)?));
  }
  Ok(Prepared {
    cwd,
    env: pretool_env(sb, c.host, extra),
    stdin,
  })
}
