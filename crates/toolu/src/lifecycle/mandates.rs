//! The optional-tool warning and the ast-grep mandate (`tool-mandates.ts`).

use toolu_protocol::host::Host;
use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::config::read::enabled;
use toolu_runtime::env::Env;

use crate::lifecycle::presence::plugin_active;
use crate::lifecycle::project::on_path;

const MISSING_AST_GREP: &str = "WARN: optional tools missing — features that depend on them are disabled:\n  • ast-grep (structural code search)";

const MANDATE: &str = concat!(
  "MANDATORY — proactive plugin use (installed + available; NOT optional, do NOT ask permission):\n  • ",
  "ast-grep (structural search) — for ANY search by code shape (signatures, call sites, impls, trait/interface usage, patterns) you MUST reach for `ast-grep run --pattern …` FIRST. Grep/ripgrep/sed are a FALLBACK ONLY — use them for plain-text literals in non-code files, or when a query genuinely cannot be expressed structurally. Never reach for them first on code.",
  "\n  • Propagation: these mandates bind EVERY agent in this session, not just the main thread. When you delegate via Task/subagents (nested ones too), carry the same requirement into the subagent's prompt — recall/save via comemory, ast-grep first with Grep/sed as fallback. Delegation never exempts the work.",
);

/// `sg` or `ast-grep` is executable on `PATH`.
pub(crate) fn has_ast_grep(env: &Env) -> bool {
  let path = env.get("PATH").unwrap_or("");
  on_path("sg", path) || on_path("ast-grep", path)
}

/// The warning when the skill is on and the tool is not on `PATH`.
pub(crate) fn missing_tools_warning(env: &Env, config: &LoadedConfig) -> Option<String> {
  if has_ast_grep(env) || !enabled(config, "skills", "ast-grep") {
    return None;
  }
  Some(MISSING_AST_GREP.to_owned())
}

/// The mandate block when ast-grep is installed, enabled and on `PATH`.
pub(crate) fn mandate_block(config: &LoadedConfig, env: &Env, host: Host) -> Option<String> {
  if has_ast_grep(env) && wanted(config, env, host) {
    Some(MANDATE.to_owned())
  } else {
    None
  }
}

fn wanted(config: &LoadedConfig, env: &Env, host: Host) -> bool {
  enabled(config, "skills", "ast-grep") && plugin_active("ast-grep@toolu", env, host)
}

#[cfg(test)]
#[path = "tests/mandates_test.rs"]
mod tests;
