//! Required and optional external tools for the installed plugins.

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_state::detect::tools::tool_available;

use super::checks::{Check, Status};
use super::inventory::{Inventory, Plugin};

struct Row {
  plugin: &'static str,
  tool: &'static str,
  /// Other tools that also satisfy the row, checked as `tool OR any of these`.
  alternatives: &'static [&'static str],
  required: bool,
  hint: &'static str,
}

impl Row {
  /// `plugin needs tool` or `plugin needs tool or alt1 or alt2`.
  fn needs(&self) -> String {
    if self.alternatives.is_empty() {
      format!("{} needs {}", self.plugin, self.tool)
    } else {
      let mut names = vec![self.tool];
      names.extend(self.alternatives);
      format!("{} needs {}", self.plugin, names.join(" or "))
    }
  }

  /// Whether the tool or any alternative is on `PATH`.
  fn available(&self, env: &Env) -> bool {
    tool_available(self.tool, env) || self.alternatives.iter().any(|alt| tool_available(alt, env))
  }
}

const ROWS: &[Row] = &[
  Row {
    plugin: "toolu",
    tool: "git",
    alternatives: &[],
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "pr-babysit",
    tool: "gh",
    alternatives: &[],
    required: true,
    hint: "install the GitHub CLI: https://cli.github.com",
  },
  Row {
    plugin: "pr-babysit",
    tool: "git",
    alternatives: &[],
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "pr-babysit",
    tool: "herdr",
    alternatives: &["tmux", "hc"],
    required: false,
    hint: "install herdr, hc or tmux",
  },
  Row {
    plugin: "epic-orchestrator",
    tool: "gh",
    alternatives: &[],
    required: true,
    hint: "install the GitHub CLI: https://cli.github.com",
  },
  Row {
    plugin: "epic-orchestrator",
    tool: "git",
    alternatives: &[],
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "epic-orchestrator",
    tool: "herdr",
    alternatives: &["tmux", "hc"],
    required: true,
    hint: "install herdr, hc or tmux",
  },
  Row {
    plugin: "toolu-review",
    tool: "git",
    alternatives: &[],
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "toolu-review",
    tool: "gh",
    alternatives: &[],
    required: false,
    hint: "install the GitHub CLI: https://cli.github.com",
  },
  Row {
    plugin: "code-review",
    tool: "git",
    alternatives: &[],
    required: true,
    hint: "install git",
  },
  Row {
    plugin: "ast-grep",
    tool: "ast-grep",
    alternatives: &[],
    required: true,
    hint: "install ast-grep: https://ast-grep.github.io",
  },
  Row {
    plugin: "python-quality",
    tool: "python3",
    alternatives: &[],
    required: false,
    hint: "install Python 3",
  },
  Row {
    plugin: "python-quality",
    tool: "ast-grep",
    alternatives: &[],
    required: false,
    hint: "install ast-grep: https://ast-grep.github.io",
  },
  Row {
    plugin: "rust-quality",
    tool: "cargo",
    alternatives: &[],
    required: false,
    hint: "install Rust: https://rustup.rs",
  },
  Row {
    plugin: "rust-quality",
    tool: "ast-grep",
    alternatives: &[],
    required: false,
    hint: "install ast-grep: https://ast-grep.github.io",
  },
  Row {
    plugin: "ts-quality",
    tool: "node",
    alternatives: &[],
    required: false,
    hint: "install Node.js",
  },
];

/// The rows that apply on macOS only: the sandbox's default backend there is a
/// Lima VM, while on Linux it runs on the machine itself.
const MACOS_ROWS: &[Row] = &[Row {
  plugin: "sandbox",
  tool: "limactl",
  alternatives: &[],
  required: false,
  hint: "install Lima 2 for toolu sandbox: brew install lima",
}];

/// Fail when a required tool is missing, else warn when an optional one is.
pub(super) fn check(inventory: &Inventory, env: &Env) -> Check {
  check_on(inventory, env, std::env::consts::OS)
}

/// [`check`] on an `os` named as `std::env::consts::OS` names it.
fn check_on(inventory: &Inventory, env: &Env, os: &str) -> Check {
  let names = installed_names(&inventory.plugins);
  let platform: &[Row] = if os == "macos" { MACOS_ROWS } else { &[] };
  let mut missing: Vec<&Row> = ROWS
    .iter()
    .chain(platform)
    .filter(|row| names.contains(&row.plugin) && !row.available(env))
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
    .map(|row| row.needs())
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
