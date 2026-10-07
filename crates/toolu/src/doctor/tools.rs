//! Required and optional external tools for the installed plugins.

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_state::detect::tools::tool_available;

use super::checks::{Check, Status};
use super::inventory::{Inventory, Plugin};

struct Row {
  plugin: &'static str,
  tool: &'static str,
  required: bool,
  hint: &'static str,
}

const ROWS: &[Row] = &[
  Row {
    plugin: "toolu",
    tool: "git",
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "pr-babysit",
    tool: "gh",
    required: true,
    hint: "install the GitHub CLI: https://cli.github.com",
  },
  Row {
    plugin: "pr-babysit",
    tool: "git",
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "pr-babysit",
    tool: "herdr",
    required: false,
    hint: "install herdr",
  },
  Row {
    plugin: "epic-orchestrator",
    tool: "gh",
    required: true,
    hint: "install the GitHub CLI: https://cli.github.com",
  },
  Row {
    plugin: "epic-orchestrator",
    tool: "git",
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "epic-orchestrator",
    tool: "herdr",
    required: true,
    hint: "install herdr",
  },
  Row {
    plugin: "toolu-review",
    tool: "git",
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "toolu-review",
    tool: "gh",
    required: false,
    hint: "install the GitHub CLI: https://cli.github.com",
  },
  Row {
    plugin: "ast-grep",
    tool: "ast-grep",
    required: true,
    hint: "install ast-grep: https://ast-grep.github.io",
  },
  Row {
    plugin: "python-quality",
    tool: "python3",
    required: false,
    hint: "install Python 3",
  },
  Row {
    plugin: "python-quality",
    tool: "ast-grep",
    required: false,
    hint: "install ast-grep: https://ast-grep.github.io",
  },
  Row {
    plugin: "rust-quality",
    tool: "cargo",
    required: false,
    hint: "install Rust: https://rustup.rs",
  },
  Row {
    plugin: "rust-quality",
    tool: "ast-grep",
    required: false,
    hint: "install ast-grep: https://ast-grep.github.io",
  },
  Row {
    plugin: "ts-quality",
    tool: "node",
    required: false,
    hint: "install Node.js",
  },
];

/// Fail when a required tool is missing, else warn when an optional one is.
pub(super) fn check(inventory: &Inventory, env: &Env) -> Check {
  let names = installed_names(&inventory.plugins);
  let mut missing: Vec<&Row> = ROWS
    .iter()
    .filter(|row| names.contains(&row.plugin) && !tool_available(row.tool, env))
    .collect();
  missing.sort_by_key(|row| !row.required);
  if missing.is_empty() {
    return Check::new(
      "tools",
      Status::Ok,
      "required tools are present",
      None,
      json!({ "missing": [] }),
    );
  }
  let required = missing.iter().any(|row| row.required);
  let summary = missing
    .iter()
    .map(|row| format!("{} needs {}", row.plugin, row.tool))
    .collect::<Vec<_>>()
    .join("; ");
  let hint = missing
    .iter()
    .find(|row| row.required)
    .or_else(|| missing.first())
    .map(|row| row.hint.to_owned());
  Check::new(
    "tools",
    if required { Status::Fail } else { Status::Warn },
    summary,
    hint,
    missing_details(&missing),
  )
}

fn installed_names(plugins: &[Plugin]) -> Vec<&str> {
  let mut names: Vec<&str> = plugins.iter().map(|plugin| plugin.name.as_str()).collect();
  names.sort_unstable();
  names.dedup();
  names
}

fn missing_details(missing: &[&Row]) -> Value {
  json!({
    "missing": missing.iter().map(|row| json!({
      "plugin": row.plugin,
      "tool": row.tool,
      "required": row.required,
    })).collect::<Vec<_>>(),
  })
}

#[cfg(test)]
#[path = "tests/tools_test.rs"]
mod tests;
