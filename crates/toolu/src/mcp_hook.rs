//! Standalone `mcp-tools` entry: run only the engine's MCP blocker.

use std::path::Path;

use serde_json::{Map, Value};
use toolu_engine::mcp_hook::check;
use toolu_protocol::decision::Decision;
use toolu_protocol::encode::{Encoded, encode};
use toolu_protocol::event::HostEvent;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::load::WARN_PREFIX;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::registry::rule::RuleContext;

fn silent() -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: None,
  }
}

fn field(doc: &Map<String, Value>, name: &str, fallback: &str) -> String {
  doc
    .get(name)
    .and_then(Value::as_str)
    .filter(|text| !text.is_empty())
    .unwrap_or(fallback)
    .to_owned()
}

fn server(tool: &str) -> Option<&str> {
  tool
    .strip_prefix("mcp__")?
    .split_once("__")
    .map(|(server, _)| server)
}

fn normalized(doc: &Map<String, Value>, tool: String, root: &Path) -> Option<NormalizedEvent> {
  let root = root.display().to_string();
  let session = Session {
    session_id: Text::new(field(doc, "session_id", "unknown")).ok()?,
    cwd: Text::new(field(doc, "cwd", &root)).ok()?,
    project_root: Text::new(root.clone()).ok()?,
    worktree: Text::new(root).ok()?,
  };
  let input = doc
    .get("tool_input")
    .and_then(Value::as_object)
    .cloned()
    .unwrap_or_default();
  let tool = Tool {
    call_id: Text::new(field(doc, "tool_use_id", "unknown")).ok()?,
    name: Text::new(tool).ok()?,
    input,
  };
  Some(NormalizedEvent::ToolPre { session, tool })
}

fn encoded(host: Host, decision: &Decision) -> Outcome {
  let target = if host == Host::Codex {
    Host::Codex
  } else {
    Host::Claude
  };
  match encode(target, HostEvent::ToolPre, decision) {
    Ok(Encoded::Command(text)) => Outcome {
      exit: Exit::Success,
      stdout: (!text.is_empty()).then(|| text.trim_end_matches('\n').to_owned()),
      stderr: None,
    },
    Ok(Encoded::Callback(_)) | Err(_) => silent(),
  }
}

fn finish(result: Result<Decision, String>, warnings: Vec<String>, host: Host) -> Outcome {
  let mut outcome = match result {
    Ok(decision) => encoded(host, &decision),
    Err(reason) => Outcome {
      exit: Exit::Blocked,
      stdout: None,
      stderr: Some(format!("blocked: {reason}")),
    },
  };
  let warning_text: String = warnings.into_iter().fold(String::new(), |mut out, line| {
    out.push_str(WARN_PREFIX);
    out.push_str(&line);
    out.push('\n');
    out
  });
  if !warning_text.is_empty() {
    outcome.stderr = Some(
      format!("{warning_text}{}", outcome.stderr.unwrap_or_default())
        .trim_end_matches('\n')
        .to_owned(),
    );
  }
  outcome
}

fn evaluate(payload: &str, plugin_root: Option<&Path>, env: Env, cwd: &Path) -> Outcome {
  let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(payload) else {
    return silent();
  };
  let tool = field(&doc, "tool_name", "");
  if server(&tool).is_none() {
    return silent();
  }
  let host = detect(&env, None).host;
  let roots = Roots::new(env.clone(), Some(host));
  let project = roots.project_root(None);
  let env = project.as_ref().map_or(env.clone(), |root| {
    env.with("TOOLU_PROJECT_DIR", &root.display().to_string())
  });
  let roots = Roots::new(env, Some(host));
  let root = project.unwrap_or_else(|| cwd.to_path_buf());
  let Some(event) = normalized(&doc, tool, &root) else {
    return silent();
  };
  let config_root = roots.config_root();
  let ctx = RuleContext {
    host,
    env: roots.env(),
    config_root: &config_root,
    project_root: &root,
    cwd: None,
    plugin_root,
    raw: &doc,
    edit: None,
  };
  let (result, warnings) = check(&event, &ctx);
  finish(result, warnings, host)
}

/// Run one standalone MCP hook call; malformed payloads allow silently.
pub fn hook(payload: std::io::Result<String>, plugin_root: Option<&Path>) -> Outcome {
  let (Ok(payload), Ok(cwd)) = (payload, current_dir()) else {
    return silent();
  };
  evaluate(&payload, plugin_root, Env::process(), &cwd)
}

#[cfg(test)]
#[path = "tests/mcp_hook_test.rs"]
mod tests;
