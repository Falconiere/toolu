//! Herdr socket client and the CLI used when the server protocol is newer.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::client::{read_json, read_text, write_json};
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
  /// `notification.show` with the `request` sound.
  Notify { title: String, body: String },
  /// `workspace.list`.
  WorkspaceList,
  /// `worktree.list` for the repository at `cwd`.
  WorktreeList { cwd: String },
  /// `worktree.open` of an existing worktree.
  WorktreeOpen {
    cwd: String,
    path: String,
    label: String,
  },
  /// `pane.list` of one workspace.
  PaneList { workspace_id: String },
  /// `pane.process_info`.
  PaneInfo { pane_id: String },
  /// `agent.list`.
  AgentList,
  /// `agent.send_keys`.
  SendKeys { target: String, keys: Vec<String> },
  /// `agent.wait` until one of `until`.
  Wait {
    target: String,
    until: Vec<String>,
    timeout_ms: u64,
  },
  /// `agent.prompt` that waits until one of `until`.
  PromptWait {
    target: String,
    text: String,
    until: Vec<String>,
    timeout_ms: u64,
  },
}

/// Protocol decision for one engine process.
#[derive(Debug, Default)]
pub(crate) struct Client {
  newer: bool,
  warned: bool,
  seq: u64,
  last_exit: Option<i32>,
  /// The resolved backend, cached after the first call so one `Client` does
  /// not flap between herdr and tmux mid-session.
  backend: Option<toolu_mux::Backend>,
}

impl Client {
  /// Honor the backend saved with the worker, even after configuration changes.
  pub(crate) fn pin(&mut self, backend: toolu_mux::Backend) {
    self.backend = Some(backend);
  }
  /// The backend selected for this client, if a call or pin selected one.
  pub(crate) fn backend(&self) -> Option<toolu_mux::Backend> {
    self.backend
  }

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
    self.last_exit = None;
    let backend = *self.backend.get_or_insert_with(|| resolve_backend(env));
    if let Some(reply) = crate::hc_backend::execute(paths, env, backend, call) {
      return reply;
    }
    if backend == toolu_mux::Backend::Tmux {
      return Ok(crate::tmux_backend::execute(call, env));
    }
    if self.newer {
      return self.cli(paths, env, call);
    }
    match self.on_socket(paths, env, call) {
      Ok(body) => Ok(body),
      Err(_) if self.newer => self.cli(paths, env, call),
      Err(err) => Err(err),
    }
  }

  /// The exit status of the last call's CLI form, when it ran through the CLI.
  pub(crate) fn last_exit(&self) -> Option<i32> {
    self.last_exit
  }

  fn on_socket(&mut self, paths: &Paths, env: &Env, call: &Call) -> Result<String, String> {
    // herdr answers one request per connection, so the call gets its own.
    let protocol = ping(&mut connect(env, call)?, &self.next_id())?;
    if protocol > HERDR_PROTOCOL {
      self.newer = true;
      warn_protocol(paths, protocol, &mut self.warned)?;
      return Err("newer protocol".to_owned());
    }
    let mut stream = connect(env, call)?;
    let id = self.next_id();
    // Tag the failing phase so `attention_ops::uncertain` can tell a post-write
    // `read:` connection reset (maybe delivered) from a `write:` / connect failure.
    write_json(
      &mut stream,
      &json!({"id": id, "method": call.method(), "params": call.params()}),
    )
    .map_err(|err| format!("write: {err}"))?;
    read_text(&mut stream).map_err(|err| format!("read: {err}"))
  }

  fn cli(&mut self, paths: &Paths, env: &Env, call: &Call) -> Result<String, String> {
    let argv = call.argv(env);
    journal_cli(paths, &argv)?;
    let mut spec = Spec::new(argv);
    spec.env = Some(env.clone());
    spec.timeout = call.cli_timeout();
    let output = run(&spec).map_err(|err| format!("{err:?}"))?;
    if output.timed_out {
      return Err("timed out".to_owned());
    }
    self.last_exit = Some(output.exit_code);
    let text = if output.stdout.trim().is_empty() {
      output.stderr
    } else {
      output.stdout
    };
    // A herdr error object carries its own code for the caller to read; any
    // other non-zero exit fails with the exit status.
    let error_object = serde_json::from_str::<Value>(text.trim())
      .is_ok_and(|reply| reply.get("error").is_some_and(Value::is_object));
    if output.exit_code != 0 && !error_object {
      return Err(format!("failed ({}): {}", output.exit_code, text.trim()));
    }
    Ok(text)
  }

  fn next_id(&mut self) -> String {
    self.seq = self.seq.saturating_add(1);
    self.seq.to_string()
  }
}

/// A new connection to the herdr socket that waits up to `call`'s deadline.
fn connect(env: &Env, call: &Call) -> Result<UnixStream, String> {
  let stream = UnixStream::connect(socket_path(env)).map_err(|err| err.to_string())?;
  stream
    .set_read_timeout(Some(call.cli_timeout()))
    .map_err(|err| err.to_string())?;
  Ok(stream)
}

/// Where the herdr API socket lives for `env`.
pub(crate) fn socket_path(env: &Env) -> PathBuf {
  toolu_mux::detect::socket_path(env)
}

/// The herdr session name from the env.
pub(crate) fn session_name(env: &Env) -> Option<&str> {
  toolu_mux::detect::session_name(env)
}

/// The configured pane manager, from `TOOLU_EPIC_MULTIPLEXER` ("auto" when
/// unset or unrecognized), resolved against `env`'s herdr reachability.
fn resolve_backend(env: &Env) -> toolu_mux::Backend {
  let configured = match env.get("TOOLU_EPIC_MULTIPLEXER") {
    Some("herdr") => toolu_mux::Multiplexer::Herdr,
    Some("herdr-app") => toolu_mux::Multiplexer::HerdrApp,
    Some("tmux") => toolu_mux::Multiplexer::Tmux,
    Some(_) | None => toolu_mux::Multiplexer::Auto,
  };
  toolu_mux::resolve(configured, env)
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

#[cfg(test)]
#[path = "tests/herdr_test.rs"]
mod tests;
