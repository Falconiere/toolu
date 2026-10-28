//! `OpenCode` V2's tools under Claude's names (<https://opencode.ai/v2/docs/tools/>).
//! A tool with a Claude counterpart takes Claude's name, and its input gains
//! Claude's key beside each `OpenCode` key, so a gate reads it as it would on
//! Claude Code. `patch` becomes Codex's `apply_patch`, whose text the edit split
//! reads from `command`. Any other tool, MCP tools included, keeps its name.

use serde_json::{Map, Value};

/// One `OpenCode` tool: its name, its Claude name, and each input key's Claude key.
struct Tool {
  opencode: &'static str,
  claude: &'static str,
  keys: &'static [(&'static str, &'static str)],
}

const PATH: (&str, &str) = ("path", "file_path");

const TOOLS: [Tool; 12] = [
  Tool {
    opencode: "shell",
    claude: "Bash",
    keys: &[("background", "run_in_background")],
  },
  Tool {
    opencode: "read",
    claude: "Read",
    keys: &[PATH],
  },
  Tool {
    opencode: "write",
    claude: "Write",
    keys: &[PATH],
  },
  Tool {
    opencode: "edit",
    claude: "Edit",
    keys: &[
      PATH,
      ("oldString", "old_string"),
      ("newString", "new_string"),
      ("replaceAll", "replace_all"),
    ],
  },
  Tool {
    opencode: "patch",
    claude: "apply_patch",
    keys: &[("patchText", "command")],
  },
  Tool {
    opencode: "glob",
    claude: "Glob",
    keys: &[],
  },
  Tool {
    opencode: "grep",
    claude: "Grep",
    keys: &[("include", "glob")],
  },
  Tool {
    opencode: "webfetch",
    claude: "WebFetch",
    keys: &[],
  },
  Tool {
    opencode: "websearch",
    claude: "WebSearch",
    keys: &[],
  },
  Tool {
    opencode: "skill",
    claude: "Skill",
    keys: &[("name", "skill")],
  },
  Tool {
    opencode: "subagent",
    claude: "Agent",
    keys: &[],
  },
  Tool {
    opencode: "question",
    claude: "AskUserQuestion",
    keys: &[],
  },
];

fn find(name: &str) -> Option<&'static Tool> {
  TOOLS.iter().find(|tool| tool.opencode == name)
}

/// The Claude name of the `OpenCode` tool `name`, else `name` itself.
pub(crate) fn claude_name(name: &str) -> &str {
  find(name).map_or(name, |tool| tool.claude)
}

/// `input` with Claude's key added beside each `OpenCode` key it renames; a key
/// already present is never overwritten.
pub(crate) fn claude_input(name: &str, input: &Map<String, Value>) -> Map<String, Value> {
  let mut out = input.clone();
  for (from, to) in find(name).map_or(&[][..], |tool| tool.keys) {
    if let Some(value) = input.get(*from) {
      out.entry(*to).or_insert_with(|| value.clone());
    }
  }
  out
}

#[cfg(test)]
#[path = "tests/tools_test.rs"]
mod tests;
