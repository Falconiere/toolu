//! The `--json` documents and the human text of the report verbs. Every document
//! names its namespace and verb; the shapes are `$defs/sandboxStatus`,
//! `sandboxSetup` and `sandboxVm` in `commands.schema.json`.

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;

use crate::context::Context;
use crate::settings::Resolved;
use crate::step::Report;
use crate::vendors::VENDORS;

/// `status`: the backend, its `state`, why it cannot run when it cannot, the
/// settings and the project.
pub(crate) fn status(context: &Context, state: &str, reason: Option<&str>) -> Report {
  let settings = &context.settings;
  let project = &context.project;
  let doc = json!({
    "namespace": "sandbox",
    "verb": "status",
    "backend": settings.backend.name(),
    "state": state,
    "reason": reason,
    "settings": settings_doc(settings),
    "project": {
      "cwd": project.cwd.to_string_lossy(),
      "root": project.root.as_ref().map(|root| root.to_string_lossy()),
    },
  });
  let reason = reason
    .map(|reason| format!(": {reason}"))
    .unwrap_or_default();
  let root = project.root.as_ref().map_or_else(
    || "not a git checkout".to_owned(),
    |root| root.display().to_string(),
  );
  let lines = [
    format!(
      "sandbox: {} backend, {state}{reason}",
      settings.backend.name()
    ),
    format!(
      "  settings: {} cpus, {} GiB memory, {} GiB disk, VM name {}",
      settings.cpus, settings.memory_gib, settings.disk_gib, settings.name
    ),
    format!("  project: {root}"),
  ];
  Report {
    exit: Exit::Success,
    doc,
    text: lines.join("\n"),
  }
}

/// `setup`: what is still missing (exit 69 while anything is), and how to install
/// and log in to each agent CLI.
pub(crate) fn setup(context: &Context, state: &str, missing: &[&str]) -> Report {
  let next: Vec<Value> = VENDORS
    .iter()
    .map(|vendor| json!({ "cli": vendor.cli, "install": vendor.install, "login": vendor.login }))
    .collect();
  let doc = json!({
    "namespace": "sandbox",
    "verb": "setup",
    "backend": context.settings.backend.name(),
    "state": state,
    "missing": missing,
    "next": next,
  });
  let mut lines = vec![
    if missing.is_empty() {
      format!("sandbox: {} backend ready", context.settings.backend.name())
    } else {
      format!("sandbox: missing on this machine: {}", missing.join(", "))
    },
    "  agent CLIs (install, then log in):".to_owned(),
  ];
  lines.extend(VENDORS.iter().map(|vendor| {
    format!(
      "  {}: {}; then {}",
      vendor.cli, vendor.install, vendor.login
    )
  }));
  let exit = if missing.is_empty() {
    Exit::Success
  } else {
    Exit::Unavailable
  };
  Report {
    exit,
    doc,
    text: lines.join("\n"),
  }
}

/// `start`, `stop` or `delete`: the state after the verb, whether it changed
/// anything, and `message` for a person.
pub(crate) fn vm(
  context: &Context,
  verb: &str,
  state: &str,
  changed: bool,
  message: &str,
) -> Report {
  Report {
    exit: Exit::Success,
    doc: json!({
      "namespace": "sandbox",
      "verb": verb,
      "backend": context.settings.backend.name(),
      "state": state,
      "changed": changed,
    }),
    text: format!("sandbox: {message}"),
  }
}

fn settings_doc(settings: &Resolved) -> Value {
  json!({
    "name": settings.name,
    "cpus": settings.cpus,
    "memory_gib": settings.memory_gib,
    "disk_gib": settings.disk_gib,
    "template": settings.template,
    "mounts": settings
      .mounts
      .iter()
      .map(|mount| mount.to_string_lossy())
      .collect::<Vec<_>>(),
  })
}

#[cfg(test)]
#[path = "tests/present_test.rs"]
mod tests;
