//! Herdr socket client and the CLI used when the server protocol is newer.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::client::{read_json, write_json};
use crate::journal::{self, Record};
use crate::paths::Paths;

/// Herdr protocol this client speaks. A higher number uses the CLI.
pub(crate) const HERDR_PROTOCOL: u64 = 22;

/// One control call. The subscription thread does not use this connection.
#[derive(Debug, Clone)]
pub(crate) enum Call {
  /// `agent.start`.
  Start {
    name: String,
    kind: String,
    pane_id: String,
    args: Vec<String>,
    timeout_ms: Option<u64>,
  },
  /// `agent.prompt` with no wait.
  Prompt { target: String, text: String },
  /// `agent.read` of the recent unwrapped tail.
  Read { target: String },
  /// `worktree.create` for each field that is set.
  Create {
    cwd: Option<String>,
    branch: Option<String>,
    base: Option<String>,
    path: Option<String>,
    label: Option<String>,
    workspace: Option<String>,
  },
  /// `worktree.remove`.
  Remove { workspace_id: String, force: bool },
}

/// Protocol decision for one engine process.
#[derive(Debug, Default)]
pub(crate) struct Client {
  newer: bool,
  warned: bool,
  seq: u64,
}

impl Client {
  /// A client that still has to read the server protocol.
  pub(crate) fn new() -> Self {
    // Construct every control call so the socket and CLI arms stay live.
    let _controls = crate::herdr_argv::sample_calls();
    Self::default()
  }

  /// Run `call` on the socket, or the CLI when the protocol is newer than 22.
  ///
  /// # Errors
  /// The socket, the journal, or the CLI command fails.
  pub(crate) fn execute(
    &mut self,
    paths: &Paths,
    env: &Env,
    call: &Call,
  ) -> Result<String, String> {
    if self.newer {
      return Self::cli(paths, env, call);
    }
    match self.on_socket(paths, env, call) {
      Ok(body) => Ok(body),
      Err(_) if self.newer => Self::cli(paths, env, call),
      Err(err) => Err(err),
    }
  }

  fn on_socket(&mut self, paths: &Paths, env: &Env, call: &Call) -> Result<String, String> {
    let mut stream = UnixStream::connect(socket_path(env)).map_err(|err| err.to_string())?;
    let protocol = ping(&mut stream, &self.next_id())?;
    if protocol > HERDR_PROTOCOL {
      self.newer = true;
      warn_protocol(paths, protocol, &mut self.warned)?;
      return Err("newer protocol".to_owned());
    }
    let id = self.next_id();
    let reply = roundtrip(&mut stream, &id, call.method(), &call.params())?;
    Ok(reply.to_string())
  }

  fn cli(paths: &Paths, env: &Env, call: &Call) -> Result<String, String> {
    let argv = call.argv(env);
    journal_cli(paths, &argv)?;
    let mut spec = Spec::new(argv);
    spec.env = Some(env.clone());
    spec.timeout = Duration::from_secs(10);
    let output = run(&spec).map_err(|err| format!("{err:?}"))?;
    if output.exit_code == 0 {
      Ok(output.stdout)
    } else {
      Err(output.stderr)
    }
  }

  fn next_id(&mut self) -> String {
    self.seq = self.seq.saturating_add(1);
    self.seq.to_string()
  }
}

/// Where the herdr API socket lives for `env`.
pub(crate) fn socket_path(env: &Env) -> PathBuf {
  if let Some(path) = env.get("HERDR_SOCKET_PATH") {
    return PathBuf::from(path);
  }
  let config = config_dir(env);
  match session_name(env) {
    Some(name) => config.join("sessions").join(name).join("herdr.sock"),
    None => config.join("herdr.sock"),
  }
}

fn config_dir(env: &Env) -> PathBuf {
  match env.get("HERDR_CONFIG_PATH") {
    Some(path) => Path::new(path)
      .parent()
      .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
    None => env.home().join(".config/herdr"),
  }
}

pub(crate) fn session_name(env: &Env) -> Option<&str> {
  env
    .get("TOOLU_EPIC_HERDR_SESSION")
    .or_else(|| env.get("HERDR_SESSION"))
}

fn ping(stream: &mut UnixStream, id: &str) -> Result<u64, String> {
  let reply = roundtrip(stream, id, "ping", &json!({}))?;
  Ok(
    reply
      .get("result")
      .and_then(|result| result.get("protocol"))
      .and_then(Value::as_u64)
      .unwrap_or(0),
  )
}

/// One herdr request and its reply.
///
/// # Errors
/// The write fails or the reply is not JSON.
pub(crate) fn roundtrip(
  stream: &mut UnixStream,
  id: &str,
  method: &str,
  params: &Value,
) -> Result<Value, String> {
  write_json(
    stream,
    &json!({"id": id, "method": method, "params": params}),
  )?;
  read_json(stream)
}

fn warn_protocol(paths: &Paths, protocol: u64, warned: &mut bool) -> Result<(), String> {
  if *warned {
    return Ok(());
  }
  *warned = true;
  let note = format!("protocol {protocol}");
  append(
    paths,
    &Record::new("action", "herdr-protocol", "", "", &note),
  )
}

fn journal_cli(paths: &Paths, argv: &[String]) -> Result<(), String> {
  append(
    paths,
    &Record::new("action", "herdr-cli", "", "", &argv.join(" ")),
  )
}

fn append(paths: &Paths, record: &Record) -> Result<(), String> {
  journal::append(
    &paths.journal_dir(),
    &paths.journal_lock(),
    record,
    SystemTime::now(),
  )
  .map(|_| ())
}

impl Call {
  fn method(&self) -> &'static str {
    match self {
      Self::Start { .. } => "agent.start",
      Self::Prompt { .. } => "agent.prompt",
      Self::Read { .. } => "agent.read",
      Self::Create { .. } => "worktree.create",
      Self::Remove { .. } => "worktree.remove",
    }
  }

  fn params(&self) -> Value {
    match self {
      Self::Start {
        name,
        kind,
        pane_id,
        args,
        timeout_ms,
      } => json!({
        "name": name,
        "kind": kind,
        "pane_id": pane_id,
        "args": args,
        "timeout_ms": timeout_ms,
      }),
      Self::Prompt { target, text } => json!({"target": target, "text": text}),
      Self::Read { target } => json!({
        "target": target,
        "source": "recent_unwrapped",
        "lines": 15,
        "format": "text",
      }),
      Self::Create {
        cwd,
        branch,
        base,
        path,
        label,
        workspace,
      } => json!({
        "cwd": cwd,
        "branch": branch,
        "base": base,
        "path": path,
        "label": label,
        "workspace": workspace,
      }),
      Self::Remove {
        workspace_id,
        force,
      } => json!({"workspace_id": workspace_id, "force": force}),
    }
  }
}

#[cfg(test)]
#[path = "tests/herdr_test.rs"]
mod tests;
