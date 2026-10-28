//! Per-plugin and dependency findings for one host.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest;
use toolu_runtime::skew::{self, Skew};
use toolu_runtime::version::{Comparison, compare};

use super::{Finding, Scope};
use crate::catalog::Catalog;
use crate::cursor;
use crate::dispatch::Context;
use crate::host::adapter::Installed;
use crate::host::{Target, muse};
use crate::verbs::{self, foreign_step, retired_step};

/// Findings for one installed plugin.
pub(super) fn assess(
  target: Target,
  entry: &Installed,
  catalog: &Catalog,
  cx: &Context<'_>,
) -> Vec<Finding> {
  if catalog.entry(&entry.name).is_none() {
    let step = retired_step(&entry.name, target.name(), Vec::new());
    return vec![Finding::warn(
      Scope::Plugin,
      Some(entry.name.clone()),
      step.detail,
      Some(remove_fix(&entry.name, target.name())),
    )];
  }
  if let Some(finding) = muse_foreign(target, entry, cx) {
    return vec![finding];
  }
  let mut findings = Vec::new();
  if !entry.enabled {
    let fix = (target == Target::Muse).then(|| format!("muse plugins enable {}", entry.name));
    findings.push(Finding::warn(
      Scope::Plugin,
      Some(entry.name.clone()),
      format!("{} is disabled", entry.version),
      fix,
    ));
  }
  findings.extend(version_and_protocol(target, entry, cx));
  if target == Target::Muse {
    findings.extend(muse_approval(entry, cx));
  }
  if findings.is_empty() {
    findings.push(Finding::ok(
      Scope::Plugin,
      Some(entry.name.clone()),
      format!("{} matches", entry.version),
    ));
  }
  findings
}

/// Missing catalog dependencies among `installed`.
pub(super) fn deps(
  target: Target,
  catalog: &Catalog,
  installed: &BTreeSet<String>,
) -> Vec<Finding> {
  let mut missing = BTreeSet::new();
  for name in installed {
    let Some(entry) = catalog.entry(name) else {
      continue;
    };
    for dep in &entry.dependencies {
      if !installed.contains(dep) {
        missing.insert(dep.clone());
      }
    }
  }
  missing
    .into_iter()
    .map(|dep| {
      Finding::warn(
        Scope::Deps,
        Some(dep.clone()),
        format!("{dep} is required by an installed plugin but is not installed"),
        Some(format!(
          "toolu plugins install {dep} --host {}",
          target.name()
        )),
      )
    })
    .collect()
}

/// Remove command for a retired plugin on `host`.
pub(super) fn remove_fix(name: &str, host: &str) -> String {
  if host == "muse" {
    format!("muse plugins remove {name}")
  } else {
    format!("toolu plugins remove {name} --host {host} --yes")
  }
}

fn muse_foreign(target: Target, entry: &Installed, cx: &Context<'_>) -> Option<Finding> {
  if target != Target::Muse {
    return None;
  }
  let source = entry.source.as_deref()?;
  let checkout = crate::host::checkout::Checkout::locate(&cx.env, cx.version, &cx.origin).ok()?;
  if checkout.owns(source) {
    return None;
  }
  let step = foreign_step(entry.name.as_str(), entry, Vec::new());
  Some(Finding::warn(
    Scope::Plugin,
    Some(entry.name.clone()),
    step.detail,
    Some(format!(
      "muse plugins remove {}; toolu plugins install {} --host muse",
      entry.name, entry.name
    )),
  ))
}

fn version_and_protocol(target: Target, entry: &Installed, cx: &Context<'_>) -> Vec<Finding> {
  let update = format!(
    "toolu plugins update {} --host {}",
    entry.name,
    target.name()
  );
  if let Some(root) = plugin_root(target, entry, cx) {
    return skew_on_root(entry, &root, cx, update);
  }
  version_only(entry, cx, update)
}

fn skew_on_root(entry: &Installed, root: &Path, cx: &Context<'_>, update: String) -> Vec<Finding> {
  let upgrade =
    upgrade_command(&std::env::current_exe().unwrap_or_else(|_| PathBuf::from("toolu")));
  let binary = skew::Binary {
    version: cx.version,
    protocol: HOOK_PROTOCOL,
  };
  let caller = skew::Caller {
    plugin: &entry.name,
    root,
  };
  match skew::assess(&caller, &binary, &manifest::read(root), upgrade) {
    Skew::Same => Vec::new(),
    Skew::Advise(text) => vec![Finding::warn(
      Scope::Plugin,
      Some(entry.name.clone()),
      text,
      Some(update),
    )],
    Skew::Mismatch(text) => vec![Finding::fail(
      Scope::Plugin,
      Some(entry.name.clone()),
      text,
      Some(update),
    )],
  }
}

fn version_only(entry: &Installed, cx: &Context<'_>, update: String) -> Vec<Finding> {
  if entry.version == "?" || compare(cx.version, &entry.version) == Comparison::Same {
    return Vec::new();
  }
  vec![Finding::warn(
    Scope::Plugin,
    Some(entry.name.clone()),
    format!(
      "installed {}, binary {}; update to align",
      entry.version, cx.version
    ),
    Some(update),
  )]
}

fn plugin_root(target: Target, entry: &Installed, cx: &Context<'_>) -> Option<PathBuf> {
  match target {
    Target::Cursor => {
      let path = cursor::store::Store::locate(&cx.env).path(&entry.name);
      path.is_dir().then_some(path)
    }
    Target::Muse => entry
      .source
      .as_ref()
      .map(PathBuf::from)
      .filter(|path| path.is_dir()),
    Target::Claude => claude_root(&entry.name, &cx.env),
    Target::Codex => None,
  }
}

fn claude_root(name: &str, env: &toolu_runtime::env::Env) -> Option<PathBuf> {
  let path = if let Some(path) = env.get("CLAUDE_PLUGINS_REGISTRY") {
    PathBuf::from(path)
  } else {
    let root = env
      .get("TOOLU_CONFIG_DIR")
      .or_else(|| env.get("CLAUDE_CONFIG_DIR"))
      .map_or_else(|| env.home().join(".claude"), PathBuf::from);
    root.join("plugins").join("installed_plugins.json")
  };
  let text = std::fs::read_to_string(path).ok()?;
  let value: Value = serde_json::from_str(&text).ok()?;
  let key = format!("{name}@toolu");
  let entries = value.get("plugins")?.get(&key)?.as_array()?;
  entries.iter().find_map(|entry| {
    let install = entry.get("installPath").and_then(Value::as_str)?;
    let path = PathBuf::from(install);
    path.is_dir().then_some(path)
  })
}

fn muse_approval(entry: &Installed, cx: &Context<'_>) -> Vec<Finding> {
  let argv = muse::argv(&["plugins", "inspect", &entry.name, "--json"]);
  let ran = verbs::Driver {
    adapter: crate::host::adapter::Adapter::Muse,
    env: cx.env.clone(),
    timeout: cx.timeout,
    checkout: None,
    pin: None,
    approval: crate::args::Approval::Report,
    progress: None,
  }
  .run(&argv);
  if !ran.ok() {
    return vec![Finding::warn(
      Scope::Plugin,
      Some(entry.name.clone()),
      "approval status is unavailable (muse plugins inspect failed)".to_owned(),
      Some(format!("muse plugins inspect {}", entry.name)),
    )];
  }
  match muse::pending(&ran.stdout) {
    Ok(true) => vec![Finding::warn(
      Scope::Plugin,
      Some(entry.name.clone()),
      "hooks are installed but inert; approve them",
      Some(format!("muse plugins approve {}", entry.name)),
    )],
    Ok(false) => Vec::new(),
    Err(cause) => vec![Finding::warn(
      Scope::Plugin,
      Some(entry.name.clone()),
      format!("approval status is unavailable: {cause}"),
      Some(format!("muse plugins inspect {}", entry.name)),
    )],
  }
}

#[cfg(test)]
#[path = "tests/plugin_test.rs"]
mod tests;
