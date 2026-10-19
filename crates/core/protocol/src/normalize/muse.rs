//! Muse's view: Claude's payload shape with Muse's `snake_case` tool names
//! (`docs/research/muse-tool-inventory.md`, #540). A tool with a Claude
//! counterpart takes Claude's name, so a gate matches it as it would on Claude;
//! every other name, Muse's internal tools and `mcp__…` included, passes through
//! as an unrecognized Claude tool name does.

use super::{View, hook_view};
use crate::payload::HookPayload;

/// Each Muse tool with a Claude counterpart, and that counterpart's name.
const MUSE_TOOLS: [(&str, &str); 20] = [
  ("bash", "Bash"),
  ("shell", "Bash"),
  ("read_file", "Read"),
  ("write_file", "Write"),
  ("edit_file", "Edit"),
  ("search", "Grep"),
  ("web_fetch", "WebFetch"),
  ("web_search", "WebSearch"),
  ("read_skill", "Skill"),
  ("cron_create", "CronCreate"),
  ("cron_delete", "CronDelete"),
  ("cron_list", "CronList"),
  ("monitor", "Monitor"),
  ("workflow", "Workflow"),
  ("subagent_spawn", "Agent"),
  ("subagent_cancel", "TaskStop"),
  ("work_stop", "TaskStop"),
  ("subagent_send_message", "SendMessage"),
  ("send_session_message", "SendMessage"),
  ("write_todos", "TodoWrite"),
];

/// The Claude name of the Muse tool `name`, else `name` itself.
fn tool_name(name: &str) -> &str {
  MUSE_TOOLS
    .iter()
    .find(|(muse, _)| *muse == name)
    .map_or(name, |(_, claude)| claude)
}

/// [`hook_view`] with the tool name canonicalized.
pub(super) fn view(payload: &HookPayload) -> View<'_> {
  let view = hook_view(payload);
  View {
    tool_name: view.tool_name.map(tool_name),
    ..view
  }
}

#[cfg(test)]
#[path = "tests/muse_test.rs"]
mod tests;
