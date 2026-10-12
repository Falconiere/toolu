//! Socket method and params of each herdr control call (protocol 22), the CLI
//! deadline, and one request with a typed failure (`scripts/common.ts herdr`).

use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::herdr::{Call, Client};
use crate::paths::Paths;

/// A failed herdr call. A timeout or a stalled prompt is `uncertain`: the
/// effect may have happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallError {
  /// What failed.
  pub message: String,
  /// herdr's error code, when it sent one.
  pub code: Option<String>,
  /// The herdr CLI's exit status, when the call ran through the CLI.
  pub exit_code: Option<i32>,
  /// The effect may have happened.
  pub uncertain: bool,
}

impl Call {
  /// The socket method.
  pub(crate) fn method(&self) -> &'static str {
    match self {
      Self::Start { .. } => "agent.start",
      Self::Prompt { .. } | Self::PromptWait { .. } => "agent.prompt",
      Self::Read { .. } => "agent.read",
      Self::Create { .. } => "worktree.create",
      Self::Remove { .. } => "worktree.remove",
      Self::WorkspaceList => "workspace.list",
      Self::WorktreeList { .. } => "worktree.list",
      Self::WorktreeOpen { .. } => "worktree.open",
      Self::PaneList { .. } => "pane.list",
      Self::PaneInfo { .. } => "pane.process_info",
      Self::AgentList => "agent.list",
      Self::SendKeys { .. } => "agent.send_keys",
      Self::Wait { .. } => "agent.wait",
      Self::Notify { .. } => "notification.show",
    }
  }

  /// The socket params.
  pub(crate) fn params(&self) -> Value {
    match self {
      Self::Start {
        name,
        kind,
        pane_id,
        args,
        timeout_ms,
      } => {
        json!({"name": name, "kind": kind, "pane_id": pane_id, "args": args, "timeout_ms": timeout_ms})
      }
      Self::Prompt { target, text } => json!({"target": target, "text": text}),
      Self::PromptWait {
        target,
        text,
        until,
        timeout_ms,
      } => {
        json!({"target": target, "text": text, "wait": {"until": until, "timeout_ms": timeout_ms}})
      }
      Self::Read { target } => json!({
        "target": target, "source": "recent_unwrapped", "lines": 15, "format": "text",
      }),
      Self::Create { .. } => self.create_params(),
      Self::Remove {
        workspace_id,
        force,
      } => json!({"workspace_id": workspace_id, "force": force}),
      Self::WorkspaceList | Self::AgentList => json!({}),
      Self::WorktreeList { cwd } => json!({"cwd": cwd}),
      Self::WorktreeOpen { cwd, path, label } => {
        json!({"cwd": cwd, "path": path, "label": label, "focus": false})
      }
      Self::PaneList { workspace_id } => json!({"workspace_id": workspace_id}),
      Self::PaneInfo { pane_id } => json!({"pane_id": pane_id}),
      Self::SendKeys { target, keys } => json!({"target": target, "keys": keys}),
      Self::Wait {
        target,
        until,
        timeout_ms,
      } => json!({"target": target, "until": until, "timeout_ms": timeout_ms}),
      Self::Notify { title, body } => json!({"title": title, "body": body, "sound": "request"}),
    }
  }

  fn create_params(&self) -> Value {
    let Self::Create {
      cwd,
      branch,
      base,
      path,
      label,
      workspace,
    } = self
    else {
      return Value::Null;
    };
    json!({
      "cwd": cwd, "branch": branch, "base": base, "path": path,
      "label": label, "workspace_id": workspace, "focus": false,
    })
  }

  /// How long the CLI form may run: the call's own wait plus 30 s. A worktree
  /// call or a listing may take 120 s, as `scripts/common.ts` allowed them.
  pub(crate) fn cli_timeout(&self) -> Duration {
    let (wait, floor) = match self {
      Self::Start { timeout_ms, .. } => (timeout_ms.unwrap_or(0), 0),
      Self::Wait { timeout_ms, .. } | Self::PromptWait { timeout_ms, .. } => (*timeout_ms, 0),
      Self::Create { .. }
      | Self::Remove { .. }
      | Self::WorkspaceList
      | Self::WorktreeList { .. }
      | Self::WorktreeOpen { .. }
      | Self::PaneList { .. } => (0, 120_000),
      Self::Prompt { .. }
      | Self::Read { .. }
      | Self::PaneInfo { .. }
      | Self::AgentList
      | Self::SendKeys { .. }
      | Self::Notify { .. } => (0, 0),
    };
    Duration::from_millis(wait.saturating_add(30_000).max(floor))
  }
}

/// Run `call` and return its `result` object (or the whole reply when it has none).
///
/// # Errors
/// The transport fails, the reply is not a JSON object, or herdr answers an error.
pub(crate) fn request(
  client: &mut Client,
  paths: &Paths,
  env: &Env,
  call: &Call,
) -> Result<Value, CallError> {
  let name = call.method();
  let sent = client.execute(paths, env, call);
  let exit_code = client.last_exit();
  let text = sent.map_err(|message| CallError {
    uncertain: message.contains("timed out") || message.contains("TimedOut"),
    message: format!("herdr {name}: {}", message.trim()),
    code: None,
    exit_code,
  })?;
  let reply = if matches!(call, Call::Read { .. }) {
    read_result(&text)
  } else {
    result_of(name, &text)
  };
  reply.map_err(|err| CallError { exit_code, ..err })
}

/// Whether a reply `Client::execute` returned means the call happened: plain
/// CLI text does, and a JSON reply must carry no herdr error object.
///
/// # Errors
/// The reply is a herdr error object.
pub(crate) fn delivered(name: &str, text: &str) -> Result<(), String> {
  if serde_json::from_str::<Value>(text.trim()).is_err() {
    return Ok(());
  }
  result_of(name, text).map(|_| ()).map_err(|err| err.message)
}

/// An `agent.read` reply: the socket's JSON, or the CLI's `--format text`
/// output as `{"read": {"text": …}}`.
fn read_result(text: &str) -> Result<Value, CallError> {
  if serde_json::from_str::<Value>(text.trim()).is_ok() {
    return result_of("agent.read", text);
  }
  Ok(json!({"read": {"text": text}}))
}

/// The `result` of a herdr reply, or its error.
///
/// # Errors
/// The text is not a JSON object, or carries an `error` object.
pub(crate) fn result_of(name: &str, text: &str) -> Result<Value, CallError> {
  let trimmed = text.trim();
  let parsed: Value = serde_json::from_str(trimmed).map_err(|_err| CallError {
    message: format!(
      "herdr {name}: non-JSON output: {}",
      trimmed.chars().take(300).collect::<String>()
    ),
    code: None,
    exit_code: None,
    uncertain: false,
  })?;
  let Value::Object(map) = parsed else {
    return Err(CallError {
      message: format!("herdr {name}: expected object JSON"),
      code: None,
      exit_code: None,
      uncertain: false,
    });
  };
  if let Some(error) = map.get("error").filter(|error| error.is_object()) {
    let code = error.get("code").and_then(Value::as_str).map(str::to_owned);
    let message = error.get("message").and_then(Value::as_str).unwrap_or("");
    return Err(CallError {
      uncertain: matches!(code.as_deref(), Some("timeout" | "agent_prompt_stalled")),
      message: format!(
        "herdr {name}: {}: {message}",
        code.as_deref().unwrap_or("null")
      ),
      code,
      exit_code: None,
    });
  }
  Ok(match map.get("result") {
    Some(result @ Value::Object(_)) => result.clone(),
    _ => Value::Object(map),
  })
}

#[cfg(test)]
#[path = "tests/herdr_calls_test.rs"]
mod tests;
