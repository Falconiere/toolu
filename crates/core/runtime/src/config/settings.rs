//! Plugin settings files (`settings-dir.ts`, `settings-files.ts`,
//! `settings.ts`): where `plugins/toolu/settings/*` live and their typed
//! loaders, the data the gate modules read. Matching (argv, globs, prefixes)
//! stays with the gate ports.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::load::is_file;
use crate::host::roots::Roots;
use crate::json::is_js_space;

/// `bash-allowlist.txt`: explicit overrides of deny rules.
pub const BASH_ALLOWLIST: &str = "bash-allowlist.txt";
/// `bash-denylist.txt`: argv-aware deny rules.
pub const BASH_DENYLIST: &str = "bash-denylist.txt";
/// `commit-prefixes.txt`: allowed Conventional Commits types.
pub const COMMIT_PREFIXES: &str = "commit-prefixes.txt";
/// `mcp-blocklist.txt`: blocked MCP server prefixes with a redirect hint.
pub const MCP_BLOCKLIST: &str = "mcp-blocklist.txt";
/// `protected-files.txt`: extended globs guarded by the protectedFiles gate.
pub const PROTECTED_FILES: &str = "protected-files.txt";
/// `rust-unsafe-exemptions.txt`: crate names allowed to use `unsafe`.
pub const RUST_UNSAFE_EXEMPTIONS: &str = "rust-unsafe-exemptions.txt";
/// `code-edit-rules.json`: the docs a code edit must read.
pub const CODE_EDIT_RULES: &str = "code-edit-rules.json";

/// `toolu_settings_dir`: `TOOLU_SETTINGS_DIR`, then `~/.claude/settings` when it
/// is a directory, then `<plugin root>/settings` (`plugin_root`, else the host's).
pub fn settings_dir(roots: &Roots, plugin_root: Option<&Path>) -> Option<PathBuf> {
  if let Some(dir) = roots.env().get("TOOLU_SETTINGS_DIR") {
    return Some(PathBuf::from(dir));
  }
  let legacy = roots.env().home().join(".claude").join("settings");
  if legacy.is_dir() {
    return Some(legacy);
  }
  let root = plugin_root
    .map(Path::to_path_buf)
    .or_else(|| roots.plugin_root())?;
  Some(root.join("settings"))
}

/// `read_list FILE`: every line that is not blank or a `#` comment, verbatim;
/// empty when the file is absent.
///
/// # Errors
/// `<path>: <reason>` when the file exists but cannot be read, so a deny list
/// never silently reads as empty.
pub fn read_list(path: &Path) -> Result<Vec<String>, String> {
  if !is_file(path) {
    return Ok(Vec::new());
  }
  let bytes = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
  let text = String::from_utf8_lossy(&bytes);
  let body = text.strip_suffix('\n').unwrap_or(&text);
  let lines = body
    .split('\n')
    .filter(|line| {
      let rest = line.trim_start_matches(is_js_space);
      !rest.is_empty() && !rest.starts_with('#')
    })
    .map(str::to_owned)
    .collect();
  Ok(lines)
}

/// One `mcp-blocklist.txt` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpBlockEntry {
  /// The blocked server prefix, trimmed.
  pub prefix: String,
  /// The hint after ` -> `, or empty.
  pub redirect: String,
}

/// `mcp-blocklist.txt` split as `mcp-blocker.sh` did: an optional ` -> <text>`
/// redirect after the prefix; an entry with an empty prefix is dropped.
///
/// # Errors
/// As [`read_list`].
pub fn mcp_blocklist(dir: &Path) -> Result<Vec<McpBlockEntry>, String> {
  let entries = read_list(&dir.join(MCP_BLOCKLIST))?
    .into_iter()
    .map(|line| {
      let (prefix, redirect) = line.split_once(" -> ").unwrap_or((&line, ""));
      McpBlockEntry {
        prefix: prefix.trim_matches(is_js_space).to_owned(),
        redirect: redirect.to_owned(),
      }
    });
  Ok(entries.filter(|entry| !entry.prefix.is_empty()).collect())
}

/// One `code-edit-rules.json` rule.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeEditRule {
  /// The glob of edited paths the rule applies to.
  #[serde(rename = "match")]
  pub matches: String,
  /// The docs to read.
  pub docs: Vec<String>,
  /// Path globs that narrow the rule; empty means every match.
  #[serde(default, rename = "when_path_matches")]
  pub when_path_matches: Vec<String>,
  /// Docs added when a narrowing glob matches.
  #[serde(default, rename = "extra_docs")]
  pub extra_docs: Vec<String>,
}

#[derive(Deserialize)]
struct CodeEditRules {
  rules: Vec<CodeEditRule>,
}

/// `code-edit-rules.json`: no rules when absent.
///
/// # Errors
/// `<path>: <reason>` when the file does not parse or a rule is off-schema.
pub fn code_edit_rules(dir: &Path) -> Result<Vec<CodeEditRule>, String> {
  let path = dir.join(CODE_EDIT_RULES);
  if !is_file(&path) {
    return Ok(Vec::new());
  }
  let text = std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
  let parsed: CodeEditRules =
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
  Ok(parsed.rules)
}

#[cfg(test)]
#[path = "tests/settings_test.rs"]
mod tests;
