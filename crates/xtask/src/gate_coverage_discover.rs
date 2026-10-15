//! Hook discovery for `cargo xtask gate-coverage`.

use std::path::Path;

use serde_json::Value;

use crate::gate_coverage::Row;
use crate::gate_coverage_names::{builtin_gates, commands_of, dir_names, hook_module, make_id};

const PRE_PARENT: &str =
  "toolu:hooks.json:PreToolUse:hook_pre-tools:apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep";
const POST_PARENT: &str =
  "toolu:hooks.json:PostToolUse:hook_post-tools:apply_patch|Edit|Write|MultiEdit|Bash|Sh";
const AGENT_PARENT: &str = "toolu:hooks.json:PreToolUse:hook_agent-tier:spawn_agent|Agent|Task";

/// The native entry point that decides an agent's model tier.
const AGENT_TIER: &str = "crates/toolu/src/agent_tier.rs";

pub(crate) fn discover(root: &Path) -> Result<Vec<Row>, String> {
  let mut out = Vec::new();
  let mut seen = Vec::new();
  for plugin in dir_names(&root.join("plugins"))? {
    discover_hooks(root, &plugin, &mut out, &mut seen)?;
  }
  discover_builtins(root, &mut out, &mut seen)?;
  out.sort_by(|left, right| left.id.cmp(&right.id));
  Ok(out)
}

fn discover_hooks(
  root: &Path,
  plugin: &str,
  out: &mut Vec<Row>,
  seen: &mut Vec<String>,
) -> Result<(), String> {
  let rel = format!("plugins/{plugin}/hooks/hooks.json");
  let path = root.join(&rel);
  if !path.is_file() {
    return Ok(());
  }
  let text = std::fs::read_to_string(&path).map_err(|err| format!("cannot read {rel}: {err}"))?;
  let Ok(doc) = serde_json::from_str::<Value>(&text) else {
    return Ok(());
  };
  let Some(hooks) = doc.get("hooks").and_then(Value::as_object) else {
    return Ok(());
  };
  for (event, entries) in hooks {
    let Some(entries) = entries.as_array() else {
      continue;
    };
    for entry in entries {
      add_entry(
        &Site {
          plugin,
          event,
          rel: &rel,
        },
        entry,
        out,
        seen,
      )?;
    }
  }
  Ok(())
}

struct Site<'a> {
  plugin: &'a str,
  event: &'a str,
  rel: &'a str,
}

fn add_entry(
  site: &Site<'_>,
  entry: &Value,
  out: &mut Vec<Row>,
  seen: &mut Vec<String>,
) -> Result<(), String> {
  let matcher = entry.get("matcher").and_then(Value::as_str).unwrap_or("");
  for command in commands_of(entry) {
    let Some(command_or_module) = hook_module(&command) else {
      return Err(format!("{}: a hook is not the native launcher", site.rel));
    };
    push(
      out,
      seen,
      made(&Made {
        plugin: site.plugin,
        kind: "hooks.json",
        event: site.event,
        command: &command_or_module,
        matcher,
        source: site.rel,
        parent: None,
      }),
    );
  }
  Ok(())
}

fn discover_builtins(
  root: &Path,
  out: &mut Vec<Row>,
  seen: &mut Vec<String>,
) -> Result<(), String> {
  for (table, event, parent) in [
    ("PRE_TOOL", "PreToolUse", PRE_PARENT),
    ("POST_TOOL", "PostToolUse", POST_PARENT),
  ] {
    for (name, source) in builtin_gates(root, table)? {
      if !root.join(&source).is_file() {
        return Err(format!("native built-in module {name} has no {source}"));
      }
      push(
        out,
        seen,
        made(&builtin(&name, event, &source, Some(parent))),
      );
    }
  }
  if root.join(AGENT_TIER).is_file() {
    push(
      out,
      seen,
      made(&Made {
        plugin: "toolu",
        kind: "entrypoint",
        event: "PreToolUse",
        command: "agent-tier",
        matcher: "",
        source: AGENT_TIER,
        parent: Some(AGENT_PARENT),
      }),
    );
  }
  Ok(())
}

struct Made<'a> {
  plugin: &'a str,
  kind: &'a str,
  event: &'a str,
  command: &'a str,
  matcher: &'a str,
  source: &'a str,
  parent: Option<&'a str>,
}

fn builtin<'a>(
  name: &'a str,
  event: &'a str,
  source: &'a str,
  parent: Option<&'a str>,
) -> Made<'a> {
  Made {
    plugin: "toolu",
    kind: "builtin-module",
    event,
    command: name,
    matcher: "",
    source,
    parent,
  }
}

fn made(row: &Made<'_>) -> Row {
  Row {
    id: make_id(row.plugin, row.kind, row.event, row.command, row.matcher),
    source_path: row.source.to_owned(),
    plugin: row.plugin.to_owned(),
    kind: row.kind.to_owned(),
    event: row.event.to_owned(),
    matcher: row.matcher.to_owned(),
    command_or_module: row.command.to_owned(),
    host_mechanism: "native".to_owned(),
    parent_id: row.parent.map(str::to_owned),
    semantics: String::new(),
    classification: String::new(),
    support: String::new(),
    implementation_issue: None,
    implementation_status: String::new(),
    limits: String::new(),
    bash_required: false,
  }
}

fn push(out: &mut Vec<Row>, seen: &mut Vec<String>, mut row: Row) {
  if seen.iter().any(|id| id == &row.id) {
    row.id = format!("{}#{}", row.id, seen.len());
  }
  seen.push(row.id.clone());
  out.push(row);
}

#[cfg(test)]
#[path = "tests/gate_coverage_discover_test.rs"]
mod tests;
