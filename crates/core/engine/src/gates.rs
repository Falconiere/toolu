//! The native built-in gates (`@toolu/core/gates`). The first three pre-tool
//! gates (#419) check edit advice, MCP servers and protected writes. After a
//! tool (#423), `gate_status` records quality-command results and `push_waiver`
//! cashes in a push-review waiver once the push lands.

/// The code-edit-rules PreToolUse built-in.
pub(crate) mod code_edit_rules;
/// Repo-relative gate paths and pathname expansion.
pub(crate) mod gate_paths;
/// `gate-status.sh`: the quality gate's command channel.
pub(crate) mod gate_status;
/// The mcp-blocker PreToolUse built-in.
pub(crate) mod mcp_blocker;
/// Bash-style path matching for the settings-driven pre-tool gates.
pub(crate) mod pattern;
/// The protected-files PreToolUse built-in.
pub(crate) mod protected_files;
/// `push-waiver.sh`: a landed push promotes its pending waiver.
pub(crate) mod push_waiver;
/// Quality commands in a parsed command line.
pub(crate) mod quality_command;
/// The command, exit status and interrupt flag of the payload.
pub(crate) mod tool_exit;

use std::path::PathBuf;
use toolu_protocol::decision::Decision;
use toolu_protocol::event::HostEvent;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::config::gate_mode::{GateMode, gate_decision, gate_mode};
use toolu_runtime::config::load::{LoadedConfig, load};
use toolu_runtime::config::settings::settings_dir;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::ShellAnalysis;

/// Whether the tool is a shell: Cursor Agent names it `Shell`, Claude Code `Bash`.
pub(crate) fn is_shell_tool(event: &NormalizedEvent) -> bool {
  event
    .tool()
    .is_some_and(|tool| matches!(tool.name.as_str(), "Bash" | "Shell"))
}

/// The analysis of the command the payload reports, as the bash modules read it.
pub(crate) fn command_analysis(ctx: &RuleContext<'_>) -> ShellAnalysis {
  toolu_shell::analyze(&tool_exit::tool_command(ctx.raw))
}

/// The settings directory seen by a built-in, including explicit plugin-root fallback.
pub(crate) fn gate_settings_dir(ctx: &RuleContext<'_>) -> Option<PathBuf> {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  settings_dir(&roots, ctx.plugin_root)
}

/// Load config as the TypeScript gate modules do, leaving warnings with the dispatcher.
pub(crate) fn gate_config(ctx: &RuleContext<'_>) -> LoadedConfig {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  load(&roots, ctx.cwd)
}

/// An edit tool's string-valued `file_path`, or empty for any other shape.
pub(crate) fn file_path(event: &NormalizedEvent) -> &str {
  event
    .tool()
    .and_then(|tool| tool.input.get("file_path"))
    .and_then(serde_json::Value::as_str)
    .unwrap_or("")
}

/// The pre-tool mode and its host-specific ask degradation.
pub(crate) fn pre_mode(
  config: &LoadedConfig,
  name: &str,
  ctx: &RuleContext<'_>,
  shell: bool,
) -> GateMode {
  let event = if shell {
    HostEvent::ShellPre
  } else {
    HostEvent::ToolPre
  };
  gate_mode(config, name, Some(ctx.host), Some(event))
}

/// Turn a mode and its exact message into an allow, advisory, ask, or deny.
pub(crate) fn decided(mode: GateMode, message: String) -> Result<Decision, String> {
  let reason = Text::new(message).map_err(|err| err.to_string())?;
  Ok(gate_decision(mode, reason).unwrap_or(Decision::Allow))
}

#[cfg(test)]
#[path = "tests/gates_test.rs"]
mod tests;
