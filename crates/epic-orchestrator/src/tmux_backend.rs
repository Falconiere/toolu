//! Maps one herdr [`Call`] onto the plain-tmux driver, answering with the same
//! `result`/`error` JSON shapes `Client::execute`'s herdr socket and CLI paths
//! return, so [`crate::herdr_calls::request`] needs no change to use either backend.
//!
//! A `target` is always the worker's pane id (`issue.pane`), never a herdr
//! agent name, so a missing pane is reported as `agent_not_found` for the
//! agent-level calls and `pane_not_found` for the pane-level ones, matching
//! what each caller already tolerates.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_mux::status::AgentStatus;
use toolu_mux::tmux::{Tmux, TmuxError};
use toolu_runtime::env::Env;

use crate::herdr::Call;
use crate::hosts::{HostKind, parse_host_kind};

/// Run `call` against plain tmux and return herdr-shaped reply text. A tmux
/// or git failure is still `Ok`-shaped JSON text, a `{"error": {...}}` object,
/// as herdr itself would send.
pub(crate) fn execute(call: &Call, env: &Env) -> String {
  // The default socket, so a blocked worker's attach command is a plain
  // `tmux attach -t <session>`; tests override it to stay off the real server.
  let socket = env
    .get("TOOLU_EPIC_TMUX_SOCKET")
    .map(std::path::PathBuf::from);
  let tmux = Tmux::new(env, socket);
  render(dispatch(&tmux, call))
}

/// `Ok` carries the already-trimmed herdr `result` text for [`Call::Read`];
/// every other call carries the `result` object.
enum Reply {
  Object(Value),
  Text(String),
}

fn dispatch(tmux: &Tmux, call: &Call) -> Result<Reply, TmuxError> {
  dispatch_agent(tmux, call).unwrap_or_else(|| dispatch_workspace(tmux, call))
}

/// Calls whose `target` is a worker's pane id, or `None` for every other call.
fn dispatch_agent(tmux: &Tmux, call: &Call) -> Option<Result<Reply, TmuxError>> {
  Some(match call {
    Call::Start {
      kind,
      pane_id,
      args,
      ..
    } => start(tmux, kind, pane_id, args).map(Reply::Object),
    Call::Prompt { target, text } => tmux
      .prompt(target, text)
      .map(|()| Reply::Object(json!({})))
      .map_err(agent_scoped),
    Call::Read { target } => tmux
      .capture(target, 15)
      .map(Reply::Text)
      .map_err(agent_scoped),
    Call::SendKeys { target, keys } => send_keys(tmux, target, keys).map(Reply::Object),
    Call::Wait {
      target,
      until,
      timeout_ms,
    } => wait(tmux, target, until, *timeout_ms).map(|_| Reply::Object(json!({}))),
    Call::PromptWait {
      target,
      text,
      until,
      timeout_ms,
    } => prompt_wait(tmux, target, text, until, *timeout_ms).map(|_| Reply::Object(json!({}))),
    Call::Create { .. }
    | Call::Remove { .. }
    | Call::Notify { .. }
    | Call::WorkspaceList
    | Call::WorktreeList { .. }
    | Call::WorktreeOpen { .. }
    | Call::PaneList { .. }
    | Call::PaneInfo { .. }
    | Call::AgentList => return None,
  })
}

/// Calls that act on a worktree, workspace or session, not a specific agent.
/// [`dispatch`] routes every agent-level call to [`dispatch_agent`] first.
fn dispatch_workspace(tmux: &Tmux, call: &Call) -> Result<Reply, TmuxError> {
  match call {
    Call::Create {
      cwd,
      branch,
      base,
      label,
      ..
    } => create(
      tmux,
      cwd.as_ref(),
      branch.as_ref(),
      base.as_ref(),
      label.as_ref(),
    )
    .map(Reply::Object),
    Call::Remove {
      workspace_id,
      force,
    } => tmux
      .remove_worktree(workspace_id, *force)
      .map(|()| Reply::Object(json!({}))),
    Call::Notify { title, body } => notify(tmux, title, body).map(Reply::Object),
    Call::WorkspaceList => Ok(Reply::Object(json!({"workspaces": []}))),
    Call::WorktreeList { cwd } => list_worktrees(tmux, cwd).map(Reply::Object),
    Call::WorktreeOpen { path, label, .. } => tmux
      .open_worktree(Path::new(path), label)
      .map(|worktree| Reply::Object(worktree.to_value())),
    Call::PaneList { workspace_id } => pane_list(tmux, workspace_id).map(Reply::Object),
    Call::PaneInfo { pane_id } => tmux
      .pane_pid(pane_id)
      .map(|pid| Reply::Object(json!({"process_info": {"shell_pid": pid}}))),
    Call::AgentList => agent_list(tmux).map(Reply::Object),
    Call::Start { .. }
    | Call::Prompt { .. }
    | Call::Read { .. }
    | Call::SendKeys { .. }
    | Call::Wait { .. }
    | Call::PromptWait { .. } => Err(TmuxError {
      code: "tmux",
      message: "internal: an agent-level call reached dispatch_workspace".to_owned(),
    }),
  }
}

fn send_keys(tmux: &Tmux, target: &str, keys: &[String]) -> Result<Value, TmuxError> {
  let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
  tmux
    .send_keys(target, &keys)
    .map(|()| json!({}))
    .map_err(agent_scoped)
}

fn prompt_wait(
  tmux: &Tmux,
  target: &str,
  text: &str,
  until: &[String],
  timeout_ms: u64,
) -> Result<AgentStatus, TmuxError> {
  tmux.prompt(target, text).map_err(agent_scoped)?;
  wait(tmux, target, until, timeout_ms)
}

/// `agent.start`: run `kind`'s binary with `args` in `pane_id`'s shell.
fn start(tmux: &Tmux, kind: &str, pane_id: &str, args: &[String]) -> Result<Value, TmuxError> {
  let mut command = vec![host_binary(kind)];
  command.extend(args.iter().map(String::as_str));
  tmux
    .start_agent(pane_id, &command)
    .map(|()| json!({}))
    .map_err(agent_scoped)
}

/// The CLI binary tmux types into the pane for `kind`; cursor's is `cursor-agent`.
/// An unknown `kind` is typed verbatim, matching herdr's own `--kind` value.
fn host_binary(kind: &str) -> &str {
  match parse_host_kind(kind) {
    Some(HostKind::Cursor) => "cursor-agent",
    Some(HostKind::Claude | HostKind::Codex | HostKind::Muse) | None => kind,
  }
}

fn create(
  tmux: &Tmux,
  cwd: Option<&String>,
  branch: Option<&String>,
  base: Option<&String>,
  label: Option<&String>,
) -> Result<Value, TmuxError> {
  let missing = |field: &str| TmuxError {
    code: "worktree_not_found",
    message: format!("worktree.create needs {field}"),
  };
  let cwd = cwd.map(String::as_str).ok_or_else(|| missing("cwd"))?;
  let branch = branch
    .map(String::as_str)
    .ok_or_else(|| missing("branch"))?;
  let base = base.map(String::as_str).ok_or_else(|| missing("base"))?;
  let label = label.map(String::as_str).ok_or_else(|| missing("label"))?;
  tmux
    .create_worktree(Path::new(cwd), branch, base, label)
    .map(|worktree| worktree.to_value())
}

fn list_worktrees(tmux: &Tmux, cwd: &str) -> Result<Value, TmuxError> {
  let rows = tmux.list_worktrees(Path::new(cwd))?;
  let worktrees: Vec<Value> = rows
    .into_iter()
    .map(|row| {
      json!({
        "branch": row.branch,
        "path": row.path,
        "open_workspace_id": row.open_workspace_id,
      })
    })
    .collect();
  Ok(json!({"worktrees": worktrees}))
}

fn pane_list(tmux: &Tmux, workspace_id: &str) -> Result<Value, TmuxError> {
  let pane_id = tmux.first_pane(workspace_id)?;
  Ok(json!({"panes": [{"pane_id": pane_id}]}))
}

/// Every open session as a best-effort `agents[]` row. The reported `agent`
/// kind is the pane's foreground command, not the launched host: tmux has no
/// registry mapping a pane back to the CLI toolu started in it.
fn agent_list(tmux: &Tmux) -> Result<Value, TmuxError> {
  let sessions = tmux.list_sessions()?;
  let agents: Vec<Value> = sessions
    .iter()
    .filter_map(|name| agent_row(tmux, name))
    .collect();
  Ok(json!({"agents": agents}))
}

fn agent_row(tmux: &Tmux, session: &str) -> Option<Value> {
  let pane_id = tmux.first_pane(session).ok()?;
  let cwd = tmux.session_cwd(session).ok()?;
  let status = tmux.pane_status(&pane_id).unwrap_or(AgentStatus::Unknown);
  let name = session.strip_prefix("toolu-").unwrap_or(session);
  Some(json!({
    "name": name,
    "pane_id": pane_id,
    "cwd": cwd,
    "agent_status": status.as_str(),
    "terminal_title_stripped": Value::Null,
  }))
}

fn notify(tmux: &Tmux, title: &str, body: &str) -> Result<Value, TmuxError> {
  // No session, or no server at all: there is nowhere to show the message.
  let Some(session) = tmux.list_sessions().unwrap_or_default().into_iter().next() else {
    return Ok(json!({}));
  };
  tmux
    .display_message(&session, &format!("{title}: {body}"))
    .map(|()| json!({}))
}

fn wait(
  tmux: &Tmux,
  target: &str,
  until: &[String],
  timeout_ms: u64,
) -> Result<AgentStatus, TmuxError> {
  let wanted: Vec<AgentStatus> = until
    .iter()
    .filter_map(|status| parse_status(status))
    .collect();
  tmux
    .wait(target, &wanted, Duration::from_millis(timeout_ms))
    .map_err(agent_scoped)
}

fn parse_status(status: &str) -> Option<AgentStatus> {
  match status {
    "idle" => Some(AgentStatus::Idle),
    "working" => Some(AgentStatus::Working),
    "blocked" => Some(AgentStatus::Blocked),
    "done" => Some(AgentStatus::Done),
    _ => None,
  }
}

/// A target the caller treats as an agent name reports `pane_not_found` as
/// `agent_not_found`, the code those call sites already tolerate.
fn agent_scoped(err: TmuxError) -> TmuxError {
  if err.code == "pane_not_found" {
    return TmuxError {
      code: "agent_not_found",
      message: err.message,
    };
  }
  err
}

fn render(result: Result<Reply, TmuxError>) -> String {
  match result {
    Ok(Reply::Text(text)) => text,
    Ok(Reply::Object(value)) => value.to_string(),
    Err(err) => json!({"error": {"code": err.code, "message": err.message}}).to_string(),
  }
}

#[cfg(test)]
#[path = "tests/tmux_backend_test.rs"]
mod tests;
