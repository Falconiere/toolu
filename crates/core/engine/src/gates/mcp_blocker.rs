//! The mcp-blocker PreToolUse gate.

use std::fmt::Write;

use serde_json::{Map, Value};
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::{GateMode, guardrail_warning};
use toolu_runtime::config::load::{LoadedConfig, exists, is_file};
use toolu_runtime::config::read::section;
use toolu_runtime::config::settings::{MCP_BLOCKLIST, McpBlockEntry, mcp_blocklist};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;

use super::{decided, gate_config, gate_settings_dir, pre_mode};
use crate::gate::Gate;

/// The mcp-blocker built-in gate.
pub(crate) struct McpBlocker;
/// Its singleton in the ordered pre-tool table.
pub(crate) static MCP_BLOCKER: McpBlocker = McpBlocker;

fn server(tool: &str) -> Option<&str> {
  tool
    .strip_prefix("mcp__")?
    .split_once("__")
    .map(|(server, _)| server)
}

fn config_entry(key: &str) -> McpBlockEntry {
  let (prefix, redirect) = key.split_once(" -> ").unwrap_or((key, ""));
  McpBlockEntry {
    prefix: prefix.trim().to_owned(),
    redirect: redirect.to_owned(),
  }
}

fn raw_section(path: &std::path::Path) -> Option<Map<String, Value>> {
  let text = std::fs::read_to_string(path).ok()?;
  let parsed: Value = serde_json::from_str(&text).ok()?;
  parsed.as_object()?.get("mcp")?.as_object().cloned()
}

fn mcp_section(config: &LoadedConfig) -> Map<String, Value> {
  if config.invalid.is_none() {
    return section(config, "mcp").cloned().unwrap_or_default();
  }
  let mut merged = Map::new();
  for path in [Some(&config.files.user), config.files.project.as_ref()]
    .into_iter()
    .flatten()
  {
    if !is_file(path) {
      continue;
    }
    if let Some(section) = raw_section(path) {
      merged.extend(section);
    }
  }
  merged
}

fn first_match<'a>(entries: &'a [McpBlockEntry], server: &str) -> Option<&'a McpBlockEntry> {
  entries
    .iter()
    .find(|entry| !entry.prefix.is_empty() && server.starts_with(&entry.prefix))
}

struct Block<'a> {
  tool: &'a str,
  server: &'a str,
  origin: String,
  redirect: String,
}

fn reason(mode: GateMode, block: &Block<'_>) -> String {
  let mut detail = "Blocked MCP servers are ones this project has decided not to reach through an MCP bridge — usually because a CLI path exists that is auditable and scoped, where the MCP tool is neither.".to_owned();
  if !block.redirect.is_empty() {
    let _ = write!(detail, " Use instead: {}", block.redirect);
  }
  let headline = format!(
    "Claude is calling MCP tool \"{}\", on the blocked server \"{}\" ({}).",
    block.tool, block.server, block.origin
  );
  match mode {
    GateMode::Ask => guardrail_warning(&headline, &detail),
    GateMode::Advise => format!(
      "MCP server \"{}\" is blocked ({}). {detail} The call was NOT stopped — gates.mcpBlocker.mode is 'advise'.",
      block.server, block.origin
    ),
    GateMode::Block | GateMode::Off => format!(
      "MCP server \"{}\" is blocked ({}). {detail}",
      block.server, block.origin
    ),
  }
}

fn candidate(
  list: Option<&std::path::PathBuf>,
  config: &LoadedConfig,
  server: &str,
) -> Result<Option<(McpBlockEntry, bool)>, String> {
  let from_file = match list {
    Some(path) if is_file(path) => {
      first_match(&mcp_blocklist(path.parent().unwrap_or(path))?, server).cloned()
    }
    Some(_) | None => None,
  };
  if let Some(found) = from_file {
    return Ok(Some((found, true)));
  }
  let disabled: Vec<_> = mcp_section(config)
    .into_iter()
    .filter(|(_, value)| *value == Value::Bool(false))
    .map(|(key, _)| config_entry(&key))
    .collect();
  Ok(
    first_match(&disabled, server)
      .cloned()
      .map(|found| (found, false)),
  )
}

/// Evaluate the blocklist while collecting the config warnings the standalone hook prints.
pub(crate) fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Result<Decision, String> {
  let Some(tool) = event.tool() else {
    return Ok(Decision::Allow);
  };
  let Some(server) = server(tool.name.as_str()) else {
    return Ok(Decision::Allow);
  };
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let list = gate_settings_dir(ctx).map(|dir| dir.join(MCP_BLOCKLIST));
  if !list.as_deref().is_some_and(is_file) && !exists(&roots, ctx.cwd) {
    return Ok(Decision::Allow);
  }
  let config = gate_config(ctx);
  let Some((found, file)) = candidate(list.as_ref(), &config, server)? else {
    warnings.extend(config.take_warnings());
    return Ok(Decision::Allow);
  };
  let origin = if file {
    "listed in settings/mcp-blocklist.txt".to_owned()
  } else {
    format!("disabled in your toolu config (mcp.{server}=false)")
  };
  let mode = pre_mode(&config, "mcpBlocker", ctx, false);
  let block = Block {
    tool: tool.name.as_str(),
    server,
    origin,
    redirect: found.redirect,
  };
  let decision = decided(mode, reason(mode, &block));
  warnings.extend(config.take_warnings());
  decision
}

impl Gate for McpBlocker {
  fn name(&self) -> &'static str {
    "mcp-blocker"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    evaluate(event, ctx, &mut Vec::new())
  }
}

#[cfg(test)]
#[path = "tests/mcp_blocker_test.rs"]
mod tests;
