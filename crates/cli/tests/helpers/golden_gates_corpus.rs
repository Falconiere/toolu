//! A row of the shared `PreToolUse` corpus in a fresh sandbox: the toolu, ast-grep and
//! fixture plugins marked installed, ast-grep's native `SessionStart` register run, then
//! the row's config, setup and settings.

use std::fs;
use std::path::Path;

use serde_json::json;

use crate::golden_gates_model::{Capture, CorpusCase, Host, Res, Scope};
use crate::golden_gates_run::{
  Call, EnvPatch, lossy, normalized, plugin_dir, plugin_root, pretool_env, run_hook, spawn,
};
use crate::golden_gates_sandbox::{Sandbox, write_at};
use crate::golden_gates_stdin::render;

const SPECS: [&str; 3] = ["toolu@toolu", "ast-grep@toolu", "fixture@toolu"];

/// Mark plugin specs installed where Claude Code records them.
fn install_plugins(sb: &Sandbox) -> Res<()> {
  let plugins: serde_json::Map<String, serde_json::Value> = SPECS
    .iter()
    .map(|spec| ((*spec).to_owned(), json!([{"scope": "user"}])))
    .collect();
  let document = json!({"version": 2, "plugins": plugins});
  let file = sb
    .config_dir(Host::Claude, Scope::User)
    .join("plugins/installed_plugins.json");
  write_at(&file, &format!("{document}\n"))
}

/// Run ast-grep's native `SessionStart` register hook.
fn register_ast_grep(sb: &Sandbox, host: Host) -> Res<()> {
  let root = plugin_dir("ast-grep");
  let mut extra = EnvPatch::new();
  extra.insert("CLAUDE_PLUGIN_ROOT".to_owned(), Some(lossy(&root)));
  if host == Host::Codex {
    extra.insert("PLUGIN_ROOT".to_owned(), Some(lossy(&root)));
  }
  let env = pretool_env(sb, host, extra);
  let root_arg = root.to_string_lossy().into_owned();
  let args = [
    "ast-grep",
    "hook",
    "register",
    "--event",
    "SessionStart",
    "--plugin-root",
    &root_arg,
  ];
  let got = spawn(
    &args,
    &Call {
      cwd: &sb.project,
      env: &env,
      stdin: "{}",
    },
  )?;
  if got.exit_code != 0 {
    return Err(format!("register ast-grep exited {}: {}", got.exit_code, got.stderr).into());
  }
  Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Res<()> {
  fs::create_dir_all(to)?;
  for entry in fs::read_dir(from)? {
    let entry = entry?;
    let target = to.join(entry.file_name());
    if entry.file_type()?.is_dir() {
      copy_dir(&entry.path(), &target)?;
    } else {
      fs::copy(entry.path(), target)?;
    }
  }
  Ok(())
}

/// Settings: a copy of the shipped settings, then the row's files written over it.
fn settings_env(sb: &Sandbox, extra: &std::collections::BTreeMap<String, String>) -> Res<EnvPatch> {
  let dir = sb.root.join("settings");
  copy_dir(&plugin_root().join("settings"), &dir)?;
  for (file, lines) in extra {
    write_at(&dir.join(file), lines)?;
  }
  Ok(EnvPatch::from([(
    "TOOLU_SETTINGS_DIR".to_owned(),
    Some(lossy(&dir)),
  )]))
}

/// Prepare the sandbox for `c` on `host`; returns the environment extras.
fn prepare(sb: &Sandbox, host: Host, c: &CorpusCase) -> Res<EnvPatch> {
  install_plugins(sb)?;
  register_ast_grep(sb, host)?;
  if let Some(config) = &c.config {
    sb.write_config(host, Scope::Project, config)?;
  }
  let setup = match host {
    Host::Claude => &c.setup_by_host.claude,
    Host::Codex => &c.setup_by_host.codex,
  };
  sb.apply(setup, host)?;
  c.settings
    .as_ref()
    .map_or_else(|| Ok(EnvPatch::new()), |files| settings_env(sb, files))
}

fn stdin_of(sb: &Sandbox, host: Host, c: &CorpusCase) -> Res<String> {
  if let Some(raw) = &c.stdin {
    return Ok(raw.clone());
  }
  let fixture = c
    .fixture
    .as_ref()
    .ok_or_else(|| format!("{}: no fixture or stdin", c.name))?;
  let tool = sb.materialize(fixture, host)?;
  Ok(render(host, &tool, &sb.project)?.to_string())
}

/// Run corpus row `c` as `host` through `toolu hook pre-tools`.
pub(crate) fn run_corpus(c: &CorpusCase, host: Host) -> Res<Capture> {
  let sb = Sandbox::new("main")?;
  let extra = prepare(&sb, host, c)?;
  let stdin = stdin_of(&sb, host, c)?;
  let env = pretool_env(&sb, host, extra);
  let got = run_hook(
    "pre-tools",
    &Call {
      cwd: &sb.project,
      env: &env,
      stdin: &stdin,
    },
  )?;
  Ok(normalized(got, &sb.root))
}
