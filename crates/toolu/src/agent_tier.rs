//! Standalone delegation telemetry and model-tier advice. Any internal failure
//! leaves the delegation alone, as the TypeScript agent-tier entry does.

mod agent_tier_ledger;

use std::path::Path;

use toolu_engine::ledger::jq::{get, parse_json, raw};
use toolu_protocol::decision::Decision;
use toolu_protocol::encode::{Encoded, encode};
use toolu_protocol::event::HostEvent;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_protocol::text::Text;
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::gate_mode::{gate_decision, gate_mode};
use toolu_runtime::config::load::{WARN_PREFIX, load};
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::ctx::StateCtx;
use toolu_state::git::has_git;
use toolu_state::telemetry::{TelemetryEvent, telemetry_append};

use self::agent_tier_ledger::join;

fn silent() -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: None,
  }
}

fn value_text(value: &Ordered) -> String {
  match value {
    Ordered::Null | Ordered::Bool(false) => String::new(),
    Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::String(_)
    | Ordered::Array(_)
    | Ordered::Object(_) => raw(value).trim_end_matches('\n').to_owned(),
  }
}

fn input_field(doc: &Ordered, keys: &[&str]) -> String {
  let Some(input) = get(doc, "tool_input").ok() else {
    return String::new();
  };
  keys
    .iter()
    .filter_map(|key| get(input, key).ok())
    .map(value_text)
    .find(|text| !text.is_empty())
    .unwrap_or_default()
}

fn optional(text: &str) -> Option<String> {
  (!text.is_empty()).then(|| text.to_owned())
}

fn warning_lines(lines: Vec<String>) -> Option<String> {
  let mut warnings = String::new();
  for line in lines {
    warnings.push_str(WARN_PREFIX);
    warnings.push_str(&line);
    warnings.push('\n');
  }
  (!warnings.is_empty()).then_some(warnings)
}

fn advice(
  roots: &Roots,
  cwd: &Path,
  step: &str,
  tier: &str,
  model: &str,
) -> (Option<String>, Vec<String>) {
  let config = load(roots, Some(cwd));
  let mode = gate_mode(
    &config,
    "agentTier",
    Some(roots.host()),
    Some(HostEvent::ToolPre),
  );
  let reason = format!(
    "plan step \"{step}\" expects model tier \"{tier}\" but this delegation used \"{model}\""
  );
  let output = Text::new(reason)
    .ok()
    .and_then(|reason| gate_decision(mode, reason))
    .and_then(|decision: Decision| {
      let host = if roots.host() == Host::Codex {
        Host::Codex
      } else {
        Host::Claude
      };
      encode(host, HostEvent::ToolPre, &decision).ok()
    });
  let stdout = match output {
    Some(Encoded::Command(text)) if !text.is_empty() => {
      Some(text.trim_end_matches('\n').to_owned())
    }
    _ => None,
  };
  (stdout, config.take_warnings())
}

fn evaluate(payload: &str, env: Env, cwd: &Path) -> Outcome {
  let Some(doc) = parse_json(payload) else {
    return silent();
  };
  let tool = get(&doc, "tool_name").map(value_text).unwrap_or_default();
  if !matches!(tool.as_str(), "Agent" | "Task" | "spawn_agent") || !has_git(&env) {
    return silent();
  }
  let Some(root) = toolu_runtime::git::toplevel(&env, cwd) else {
    return silent();
  };
  let model = input_field(&doc, &["model"]);
  let subagent = input_field(&doc, &["subagent_type", "task_name", "agent_type"]);
  let effort = input_field(&doc, &["reasoning_effort", "reasoningEffort"]);
  let host = detect(&env, None).host;
  let roots = Roots::new(env, Some(host));
  let (step, tier) = join(&root, &roots);
  let mut state = StateCtx::new(roots.clone());
  let event = TelemetryEvent::Delegation {
    model: optional(&model),
    subagent_type: optional(&subagent),
    reasoning_effort: optional(&effort),
    step_id: optional(&step),
    step_model: optional(&tier),
  };
  let _written = telemetry_append(&mut state, &root, &event);
  let mut warnings = state.warnings;
  let stdout = if tier.is_empty() || model.is_empty() || tier == model {
    None
  } else {
    let (out, lines) = advice(&roots, cwd, &step, &tier, &model);
    warnings.extend(lines);
    out
  };
  Outcome {
    exit: Exit::Success,
    stdout,
    stderr: warning_lines(warnings),
  }
}

/// Run the standalone hook over its raw payload; an unreadable payload fails open.
pub fn hook(payload: std::io::Result<String>) -> Outcome {
  let (Ok(payload), Ok(cwd)) = (payload, current_dir()) else {
    return silent();
  };
  evaluate(&payload, Env::process(), &cwd)
}

#[cfg(test)]
#[path = "tests/agent_tier_test.rs"]
mod tests;
