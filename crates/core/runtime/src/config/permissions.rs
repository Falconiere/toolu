//! The one-time host permission allowlist (`permissions.ts`). The first toolu
//! session in a Claude repository unions an allowlist into
//! `<project>/.claude/settings.local.json` and records a sentinel under the
//! git-ignored state root, so a rule the user deletes stays deleted. A settings
//! file that cannot be parsed is left byte-identical, and the rewrite keeps the
//! file's key order. An invalid config envelope skips the write: grants are
//! never broadened from a config toolu cannot read.

use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;

use super::load::LoadedConfig;
use super::read::{flag_false, section};
use crate::atomic::write_atomic;
use crate::env::Env;
use crate::host::roots::Roots;
use crate::json::ordered::Ordered;
use crate::process::commands::is_git_repo;

/// Blanket shell plus the two edit tools; `Bash(*)` subsumes `Bash(git:*)`.
const DEFAULT_PERMISSIONS: [&str; 3] = ["Bash(*)", "Edit", "Write"];
/// The state-root file that records the write.
const PERMISSIONS_SENTINEL: &str = ".permissions-written";

/// What [`permissions_autowrite`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionsResult {
  /// The settings file was written.
  Written {
    /// The file.
    settings_file: PathBuf,
    /// The rules added, in order.
    added: Vec<String>,
    /// The one-line notice for the session, when a rule was added.
    notice: Option<String>,
  },
  /// Nothing was written, and why.
  Skipped(String),
}

/// A configured `permissions.allow` (its strings) replaces the default outright.
fn entries(config: &LoadedConfig) -> Vec<String> {
  let allow = section(config, "permissions").and_then(|permissions| permissions.get("allow"));
  let configured: Vec<String> = allow
    .and_then(serde_json::Value::as_array)
    .map(|items| {
      items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
    })
    .unwrap_or_default();
  if configured.is_empty() {
    DEFAULT_PERMISSIONS
      .iter()
      .map(|entry| (*entry).to_owned())
      .collect()
  } else {
    configured
  }
}

/// The settings file and sentinel, or why there is nothing to do.
fn target(
  config: &LoadedConfig,
  env: &Env,
  root: Option<&Path>,
  cwd: Option<&Path>,
) -> Result<(PathBuf, PathBuf), String> {
  let roots = Roots::new(env.clone(), Some(config.host));
  let root = root
    .map(Path::to_path_buf)
    .or_else(|| roots.project_root(cwd))
    .filter(|root| !root.as_os_str().is_empty());
  let root = root.ok_or("no project root")?;
  if config.host != Host::Claude {
    return Err("not claude".to_owned());
  }
  if config.invalid.is_some() {
    return Err("config invalid".to_owned());
  }
  if flag_false(config, "permissions", "autoAllow") {
    return Err("autoAllow is false".to_owned());
  }
  if !is_git_repo(env, &root) {
    return Err("not a git repository".to_owned());
  }
  let state = roots
    .project_state_root(cwd, Some(&root))
    .ok_or("no state root")?;
  let sentinel = state.join(PERMISSIONS_SENTINEL);
  if sentinel.exists() {
    return Err("already written".to_owned());
  }
  Ok((
    root
      .join(roots.project_dirname())
      .join("settings.local.json"),
    sentinel,
  ))
}

/// `jq -e .` then the merge filter's own type errors.
fn read_settings(file: &Path) -> Result<Ordered, String> {
  let untouched = || format!("malformed JSON in {}; leaving it untouched", file.display());
  let text = match std::fs::read(file) {
    Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
    Err(_) if !file.exists() => return Ok(Ordered::Object(Vec::new())),
    Err(_) => return Err(untouched()),
  };
  match Ordered::parse(&text) {
    Ok(Ordered::Null | Ordered::Bool(false)) | Err(_) => Err(untouched()),
    Ok(object @ Ordered::Object(_)) => Ok(object),
    Ok(Ordered::Bool(true) | Ordered::Number(_) | Ordered::String(_) | Ordered::Array(_)) => Err(
      format!("could not merge permissions into {}", file.display()),
    ),
  }
}

/// jq `(.permissions.allow // []) | map(select(type == "string"))`; `None` on a jq error.
fn existing_allow(settings: &Ordered) -> Option<Vec<String>> {
  let strings = |items: Vec<&Ordered>| {
    let texts = items.into_iter().filter_map(|item| match item {
      Ordered::String(text) => Some(text.clone()),
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::Array(_)
      | Ordered::Object(_) => None,
    });
    Some(texts.collect())
  };
  let permissions = match settings.get("permissions") {
    None | Some(Ordered::Null | Ordered::Bool(false)) => return Some(Vec::new()),
    Some(object @ Ordered::Object(_)) => object,
    Some(_) => return None,
  };
  match permissions.get("allow") {
    None | Some(Ordered::Null | Ordered::Bool(false)) => Some(Vec::new()),
    Some(Ordered::Array(items)) => strings(items.iter().collect()),
    Some(Ordered::Object(entries)) => strings(entries.iter().map(|(_, item)| item).collect()),
    Some(Ordered::Bool(true) | Ordered::Number(_) | Ordered::String(_)) => None,
  }
}

/// The settings with the missing rules appended to `permissions.allow`, and
/// those rules.
fn merged(config: &LoadedConfig, settings_file: &Path) -> Result<(Ordered, Vec<String>), String> {
  let mut settings = read_settings(settings_file)?;
  let have = existing_allow(&settings).ok_or_else(|| {
    format!(
      "could not merge permissions into {}",
      settings_file.display()
    )
  })?;
  let added: Vec<String> = entries(config)
    .into_iter()
    .filter(|entry| !have.contains(entry))
    .collect();
  let mut permissions = match settings.get("permissions") {
    Some(object @ Ordered::Object(_)) => object.clone(),
    Some(
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Array(_),
    )
    | None => Ordered::Object(Vec::new()),
  };
  let allow = have
    .iter()
    .chain(&added)
    .map(|entry| Ordered::String(entry.clone()))
    .collect();
  permissions.set("allow", Ordered::Array(allow));
  settings.set("permissions", permissions);
  Ok((settings, added))
}

/// The session notice naming what was added, when anything was.
fn notice(added: &[String], settings_file: &Path) -> Option<String> {
  (!added.is_empty()).then(|| {
    format!(
      "toolu wrote {} to {} (one time only; delete a rule and it stays deleted). Add that file to .gitignore if it is not there already.",
      added.join(", "),
      settings_file.display()
    )
  })
}

/// `toolu_permissions_autowrite ROOT`: unions the allowlist into the Claude
/// project's `settings.local.json` once; `root` defaults to the project root
/// found from `cwd`.
pub fn permissions_autowrite(
  config: &LoadedConfig,
  env: &Env,
  root: Option<&Path>,
  cwd: Option<&Path>,
) -> PermissionsResult {
  let (settings_file, sentinel) = match target(config, env, root, cwd) {
    Ok(target) => target,
    Err(reason) => return PermissionsResult::Skipped(reason),
  };
  let written = merged(config, &settings_file).and_then(|(settings, added)| {
    let body = format!("{}\n", settings.to_text(true));
    if write_atomic(&settings_file, &body) {
      Ok(added)
    } else {
      Err(format!("could not write {}", settings_file.display()))
    }
  });
  let added = match written {
    Ok(added) => added,
    Err(reason) => {
      config.warn(reason.clone());
      return PermissionsResult::Skipped(reason);
    }
  };
  // Written only after the settings landed, so a failure retries next session.
  let marked = sentinel
    .parent()
    .is_none_or(|dir| std::fs::create_dir_all(dir).is_ok())
    && std::fs::write(&sentinel, "").is_ok();
  if !marked {
    config.warn(format!("could not write {}", sentinel.display()));
  }
  let notice = notice(&added, &settings_file);
  PermissionsResult::Written {
    settings_file,
    added,
    notice,
  }
}

#[cfg(test)]
#[path = "tests/permissions_test.rs"]
mod tests;
