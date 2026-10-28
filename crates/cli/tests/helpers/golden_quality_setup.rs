//! Putting a sandbox in a case's starting state: committed files, installed
//! plugins, setup actions, the registered plugin, and the environment hooks see.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::Res;
use crate::cases::{Action, Case, CaseEnv, Host, Tagged};
use crate::proc;
use crate::sandbox::{Sandbox, inside, write_at};

/// Environment keys of a developer's own session that must not reach a hook.
const HOST_PREFIXES: &[&str] = &["CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_", "GIT_", "OPENCODE"];

/// The repository root.
pub(crate) fn repo_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `plugins/<name>` in this repository.
pub(crate) fn plugin_root(name: &str) -> PathBuf {
  repo_root().join("plugins").join(name)
}

/// This process's environment without the host-session keys.
fn parent_env() -> BTreeMap<String, String> {
  std::env::vars_os()
    .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
    .filter(|(key, _)| {
      key != "PLUGIN_ROOT" && !HOST_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
    })
    .collect()
}

/// The environment a host puts on a hook process, rooted in the sandbox, plus `extra`.
pub(crate) fn hook_env(
  sb: &Sandbox,
  host: Host,
  extra: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
  let toolu_plugin = plugin_root("toolu").display().to_string();
  let mut env = parent_env();
  env.insert("HOME".to_owned(), sb.home.display().to_string());
  env.insert("PATH".to_owned(), sb.path_var.clone());
  match host {
    Host::Claude => {
      env.insert(
        "CLAUDE_PROJECT_DIR".to_owned(),
        sb.project.display().to_string(),
      );
      let config = sb.user_config(Host::Claude).display().to_string();
      env.insert("CLAUDE_CONFIG_DIR".to_owned(), config);
    }
    Host::Codex => {
      env.insert("PLUGIN_ROOT".to_owned(), toolu_plugin.clone());
      env.insert("CODEX_HOME".to_owned(), sb.codex_home.display().to_string());
    }
  }
  env.insert("CLAUDE_PLUGIN_ROOT".to_owned(), toolu_plugin);
  env.extend(
    extra
      .iter()
      .map(|(key, value)| (key.clone(), value.clone())),
  );
  env
}

/// Run `plugin`'s `SessionStart` register hook, which must succeed silently.
pub(crate) fn register(sb: &Sandbox, host: Host, plugin: &str) -> Res<()> {
  let root = plugin_root(plugin).display().to_string();
  let mut extra = BTreeMap::from([("CLAUDE_PLUGIN_ROOT".to_owned(), root.clone())]);
  if host == Host::Codex {
    extra.insert("PLUGIN_ROOT".to_owned(), root.clone());
  }
  let env = hook_env(sb, host, &extra);
  let args = [
    plugin,
    "hook",
    "register",
    "--event",
    "SessionStart",
    "--plugin-root",
    &root,
  ];
  let ran = proc::toolu(&args, &sb.project, &env, "{}")?;
  if ran.code != 0 || !ran.stdout.is_empty() {
    return Err(
      format!(
        "register {plugin}: exit {} {}{}",
        ran.code, ran.stdout, ran.stderr
      )
      .into(),
    );
  }
  Ok(())
}

/// Record `specs` as installed where Claude Code keeps them.
fn install_plugins(sb: &Sandbox, specs: &[&str]) -> Res<()> {
  let plugins: serde_json::Map<String, serde_json::Value> = specs
    .iter()
    .map(|spec| ((*spec).to_owned(), json!([{"scope": "user"}])))
    .collect();
  let file = sb.home.join(".claude/plugins/installed_plugins.json");
  write_at(
    &file,
    &format!("{}\n", json!({"version": 2, "plugins": plugins})),
  )
}

fn commit_project(sb: &Sandbox, case: &Case) -> Res<()> {
  let mut committed = case.project.clone();
  committed.extend(
    case
      .commit
      .iter()
      .map(|(rel, body)| (rel.clone(), body.clone())),
  );
  for (rel, body) in &committed {
    sb.write(rel, body)?;
  }
  if committed.is_empty() {
    return Ok(());
  }
  let mut add = vec!["add", "--"];
  add.extend(committed.keys().map(String::as_str));
  sb.git(&add)?;
  sb.git(&["commit", "-q", "-m", "project"])?;
  Ok(())
}

/// Commit the case's files, install `specs`, write its config and run its setup.
pub(crate) fn prepare(sb: &Sandbox, host: Host, case: &Case, specs: &[&str]) -> Res<()> {
  // No detached gc or maintenance writing into `.git` while a hook runs.
  sb.git(&["config", "maintenance.auto", "false"])?;
  sb.git(&["config", "gc.auto", "0"])?;
  install_plugins(sb, specs)?;
  commit_project(sb, case)?;
  if let Some(config) = &case.config {
    let file = sb.project_config(host).join("toolu.config.json");
    write_at(
      &file,
      &format!("{}\n", serde_json::to_string_pretty(config)?),
    )?;
  }
  apply_setup(sb, host, &case.setup)
}

/// A sandbox path: a `$PROJECT`, `$ROOT`, `$HOME` or `$HOST_STATE` token, or project-relative.
fn token_path(sb: &Sandbox, host: Host, text: &str) -> Res<PathBuf> {
  let bases = [
    ("$PROJECT", sb.project.clone()),
    ("$ROOT", sb.root.clone()),
    ("$HOME", sb.home.clone()),
    ("$HOST_STATE", sb.project_config(host)),
  ];
  for (token, base) in &bases {
    if text == *token {
      return Ok(base.clone());
    }
    if let Some(rest) = text.strip_prefix(&format!("{token}/")) {
      return inside(base, rest);
    }
  }
  if text.starts_with('$') || Path::new(text).is_absolute() {
    return Err(format!("unknown path token or escape: {text}").into());
  }
  sb.path(text)
}

fn tagged_text(sb: &Sandbox, host: Host, value: &Tagged) -> Res<String> {
  match value {
    Tagged::Text(text) => Ok(text.clone()),
    Tagged::Path { path } => Ok(token_path(sb, host, path)?.display().to_string()),
  }
}

/// A symlink target: a sandbox token, or `$REPO/<path>` in this checkout.
fn target_path(sb: &Sandbox, host: Host, value: &Tagged) -> Res<PathBuf> {
  if let Tagged::Path { path } = value
    && let Some(rest) = path.strip_prefix("$REPO/")
  {
    return inside(&repo_root(), rest);
  }
  Ok(PathBuf::from(tagged_text(sb, host, value)?))
}

fn apply_action(sb: &Sandbox, host: Host, action: &Action) -> Res<()> {
  match action {
    Action::Write { path, body } => write_at(&token_path(sb, host, path)?, body),
    Action::Remove { path } => {
      let target = token_path(sb, host, path)?;
      match std::fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&target)?,
        Ok(_) => std::fs::remove_file(&target)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
      }
      Ok(())
    }
    Action::Mkdir { path } => Ok(std::fs::create_dir_all(token_path(sb, host, path)?)?),
    Action::Symlink { path, target } => {
      let link = token_path(sb, host, path)?;
      if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
      }
      Ok(std::os::unix::fs::symlink(
        target_path(sb, host, target)?,
        link,
      )?)
    }
    Action::Chmod { path, mode } => {
      let mode = u32::from_str_radix(mode, 8)?;
      let file = token_path(sb, host, path)?;
      Ok(std::fs::set_permissions(
        file,
        std::fs::Permissions::from_mode(mode),
      )?)
    }
    Action::Git { args } => {
      let words = args
        .iter()
        .map(|arg| tagged_text(sb, host, arg))
        .collect::<Res<Vec<_>>>()?;
      sb.git(&words.iter().map(String::as_str).collect::<Vec<_>>())?;
      Ok(())
    }
  }
}

/// Run the case's setup actions in order.
fn apply_setup(sb: &Sandbox, host: Host, actions: &[Action]) -> Res<()> {
  actions
    .iter()
    .try_for_each(|action| apply_action(sb, host, action))
}

/// `PATH` as a directory of links to everything on `PATH` but `without`.
fn path_without(sb: &Sandbox, without: &[String]) -> Res<String> {
  let bin = sb.root.join(format!("path-without-{}", without.join("-")));
  std::fs::create_dir_all(&bin)?;
  for dir in std::env::split_paths(&sb.path_var).filter(|dir| !dir.as_os_str().is_empty()) {
    let entries = match std::fs::read_dir(&dir) {
      Ok(entries) => entries,
      Err(err) if skippable(&err) => continue,
      Err(err) => return Err(err.into()),
    };
    for entry in entries {
      let name = entry?.file_name();
      let link = bin.join(&name);
      let hidden = name
        .to_str()
        .is_some_and(|name| without.iter().any(|w| w == name));
      if !hidden && std::fs::symlink_metadata(&link).is_err() {
        std::os::unix::fs::symlink(dir.join(&name), &link)?;
      }
    }
  }
  Ok(bin.display().to_string())
}

fn skippable(err: &std::io::Error) -> bool {
  use std::io::ErrorKind::{NotADirectory, NotFound, PermissionDenied};
  matches!(err.kind(), NotFound | NotADirectory | PermissionDenied)
}

/// An `ast-grep` script with `body` first on `PATH`.
fn stub_ast_grep(sb: &Sandbox, body: &str) -> Res<String> {
  let bin = sb.root.join("stub-bin");
  std::fs::create_dir_all(&bin)?;
  proc::write_script(&bin.join("ast-grep"), &format!("#!/bin/sh\n{body}\n"))?;
  Ok(format!("{}:{}", bin.display(), sb.path_var))
}

/// The environment the case's variant adds to each step.
pub(crate) fn case_env(sb: &Sandbox, env: Option<&CaseEnv>) -> Res<BTreeMap<String, String>> {
  let mut out = BTreeMap::new();
  match env {
    None => {}
    Some(CaseEnv::PathWithout { without }) => {
      out.insert("PATH".to_owned(), path_without(sb, without)?);
    }
    Some(CaseEnv::StubAstGrep { body }) => {
      out.insert("PATH".to_owned(), stub_ast_grep(sb, body)?);
    }
    Some(CaseEnv::ProjectSettings { files }) => {
      for (file, body) in files {
        sb.write(&format!("settings/{file}"), body)?;
      }
      out.insert(
        "TOOLU_SETTINGS_DIR".to_owned(),
        sb.path("settings")?.display().to_string(),
      );
    }
  }
  Ok(out)
}
