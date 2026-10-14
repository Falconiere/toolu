//! What a verb prints: the TypeScript installer's reports byte for byte
//! (the former TypeScript installer's `src/ui/report.ts`), and their `--json` documents.

use serde_json::{Map, Value, json};

use crate::host::Target;
use crate::step::{Status, Step};

/// One catalog plugin as `list` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
  /// The plugin.
  pub(crate) name: String,
  /// Whether the host has it.
  pub(crate) installed: bool,
  /// The version it has, when known.
  pub(crate) version: Option<String>,
  /// Whether it is enabled.
  pub(crate) enabled: bool,
}

fn line(mark: char, name: &str, detail: &str) -> String {
  format!("  {mark} {name:<16} {detail}\n")
}

fn marked(steps: &[Step]) -> String {
  steps
    .iter()
    .map(|step| line(step.status.mark(), &step.name, &step.detail))
    .collect()
}

/// One host's install report: outcomes, or on a dry run the planned commands.
fn install_one(steps: &[Step], dry_run: bool) -> String {
  if dry_run {
    commands(steps)
  } else {
    marked(steps)
  }
}

/// A host-CLI dry run: the commands, in order, that would run.
pub(crate) fn commands(steps: &[Step]) -> String {
  let lines = steps
    .iter()
    .map(|step| format!("  {}\n", step.argv.join(" ")))
    .collect::<Vec<_>>()
    .concat();
  format!("Planned commands (nothing was run):\n{lines}")
}

/// The install report, with a section per host when there are several.
pub(crate) fn install(sections: &[(Target, Vec<Step>)], dry_run: bool) -> String {
  if let [(_, steps)] = sections {
    return install_one(steps, dry_run);
  }
  sections
    .iter()
    .map(|(target, steps)| format!("{}:\n{}", target.name(), install_one(steps, dry_run)))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The catalog joined with the host's state.
pub(crate) fn list(rows: &[Row]) -> String {
  rows
    .iter()
    .map(|row| {
      if !row.installed {
        return line('-', &row.name, "not installed");
      }
      let version = row.version.as_deref().unwrap_or("?");
      let disabled = if row.enabled { "" } else { " (disabled)" };
      line('=', &row.name, &format!("{version}{disabled}"))
    })
    .collect()
}

/// The remove report.
pub(crate) fn remove(steps: &[Step]) -> String {
  steps
    .iter()
    .map(|step| {
      line(
        if step.failed() { 'x' } else { '-' },
        &step.name,
        &step.detail,
      )
    })
    .collect()
}

/// The update report.
pub(crate) fn update(steps: &[Step]) -> String {
  marked(steps)
}

/// A step as `--json` reports it; a removal reports `removed` instead of `outcome`.
pub(crate) fn step_json(step: &Step, removal: bool) -> Value {
  let mut doc = Map::new();
  doc.insert("name".to_owned(), json!(step.name));
  if removal {
    doc.insert("removed".to_owned(), json!(step.status == Status::Removed));
  } else {
    doc.insert("outcome".to_owned(), json!(step.status.name()));
  }
  doc.insert("detail".to_owned(), json!(step.detail));
  doc.insert("argv".to_owned(), json!(step.argv));
  Value::Object(doc)
}

/// `list --json`: one object per catalog plugin; `version` only when known.
pub(crate) fn rows_json(rows: &[Row]) -> Value {
  Value::Array(
    rows
      .iter()
      .map(|row| {
        let mut doc = Map::new();
        doc.insert("name".to_owned(), json!(row.name));
        doc.insert("installed".to_owned(), json!(row.installed));
        if let Some(version) = &row.version {
          doc.insert("version".to_owned(), json!(version));
        }
        doc.insert("enabled".to_owned(), json!(row.enabled));
        Value::Object(doc)
      })
      .collect(),
  )
}

/// `install --json`.
pub(crate) fn install_json(sections: &[(Target, Vec<Step>)], dry_run: bool) -> Value {
  let hosts: Vec<Value> = sections
    .iter()
    .map(|(target, steps)| host_json(*target, steps, false))
    .collect();
  json!({ "dryRun": dry_run, "hosts": hosts })
}

/// One host's steps as `--json` reports them.
pub(crate) fn host_json(target: Target, steps: &[Step], removal: bool) -> Value {
  let steps: Vec<Value> = steps.iter().map(|step| step_json(step, removal)).collect();
  json!({ "host": target.name(), "steps": steps })
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
