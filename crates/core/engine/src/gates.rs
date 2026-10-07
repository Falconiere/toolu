//! The native built-in gates (`@toolu/core/gates`). After a tool (#423):
//! `gate_status` records the global quality gate from a quality command whose
//! exit status is observable, and `push_waiver` cashes in a push-review waiver
//! once the push lands. Both read the payload through `tool_exit` and the
//! parsed command through `quality_command` and `crate::detect`.

/// `gate-status.sh`: the quality gate's command channel.
pub mod gate_status;
/// `push-waiver.sh`: a landed push promotes its pending waiver.
pub mod push_waiver;
/// Quality commands in a parsed command line.
pub(crate) mod quality_command;
/// The command, exit status and interrupt flag of the payload.
pub(crate) mod tool_exit;

use toolu_protocol::normalized::NormalizedEvent;
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

#[cfg(test)]
#[path = "tests/gates_test.rs"]
mod tests;
