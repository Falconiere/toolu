//! Hook discovery for `cargo xtask gate-coverage`.

use std::path::Path;

use serde_json::Value;

use crate::bun_launcher::is_native_like;
use crate::gate_coverage::Row;
use crate::gate_coverage_names::{
  commands_of, dir_names, make_id, native_names, normalize_command,
};

const PRE_PARENT: &str =
  "toolu:hooks.json:PreToolUse:pre-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep";
const POST_PARENT: &str =
  "toolu:hooks.json:PostToolUse:post-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Sh";
const AGENT_PARENT: &str = "toolu:hooks.json:PreToolUse:agent-tier.js:spawn_agent|Agent|Task";

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
    let native = command.contains("--hook-protocol");
    if !native && is_native_like(&command) {
      return Err(format!(
        "{}: native launcher is missing --hook-protocol",
        site.rel
      ));
    }
    let command_or_module = normalize_command(&command);
    let host = if native { "native" } else { "bun-bundle" };
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
        host,
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
  for name in native_names(root)? {
    let rel = format!("packages/toolu-core/src/gates/{name}.ts");
    if !root.join(&rel).is_file() {
      return Err(format!("native built-in module {name} has no {rel}"));
    }
    push(
      out,
      seen,
      made(&builtin(&name, "PreToolUse", &rel, Some(PRE_PARENT))),
    );
  }
  for name in ["gate-status", "push-waiver"] {
    let rel = format!("packages/toolu-core/src/gates/{name}.ts");
    if !root.join(&rel).is_file() {
      return Err(format!("native built-in module {name} has no {rel}"));
    }
    push(
      out,
      seen,
      made(&builtin(name, "PostToolUse", &rel, Some(POST_PARENT))),
    );
  }
  let agent = "plugins/toolu/hooks/src/agent-tier.ts";
  if root.join(agent).is_file() {
    push(
      out,
      seen,
      made(&Made {
        plugin: "toolu",
        kind: "entrypoint",
        event: "PreToolUse",
        command: "agent-tier",
        matcher: "",
        source: agent,
        host: "bun-bundle",
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
  host: &'a str,
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
    host: "bun-bundle",
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
    host_mechanism: row.host.to_owned(),
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
