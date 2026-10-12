//! A herdr 0.9.3 (protocol 22) server for tests: a real Unix socket, real git
//! worktrees and real agent processes, with scripted error objects and
//! non-JSON replies.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::herdr_state::{Scripted, State};

/// The running server and its state.
pub(crate) struct HerdrFixture {
  /// The socket path.
  pub socket: PathBuf,
  state: Arc<Mutex<State>>,
  _dir: tempfile::TempDir,
}

impl HerdrFixture {
  /// Serve protocol 22 on a fresh socket.
  pub(crate) fn start() -> Self {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("herdr.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let state = Arc::new(Mutex::new(State::new(dir.path())));
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
      for stream in listener.incoming().map_while(Result::ok) {
        let state = Arc::clone(&shared);
        std::thread::spawn(move || serve(stream, &state));
      }
    });
    Self {
      socket,
      state,
      _dir: dir,
    }
  }

  /// The state, locked.
  pub(crate) fn state(&self) -> MutexGuard<'_, State> {
    self
      .state
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// An environment that reaches this server and no herdr binary.
  pub(crate) fn env(&self) -> Env {
    Env::from_pairs([
      ("HOME", "/tmp".to_owned()),
      ("PATH", "/usr/bin:/bin".to_owned()),
      ("HERDR_SOCKET_PATH", self.socket.display().to_string()),
    ])
  }

  /// Register `repo` as a primary checkout herdr has a workspace for.
  pub(crate) fn add_repo(&self, repo: &Path) -> String {
    self.state().open(repo, repo, "repo", false)
  }

  /// Answer the next `method` call with `reply` instead of serving it.
  pub(crate) fn script(&self, method: &str, reply: Scripted) {
    self
      .state()
      .scripted
      .entry(method.to_owned())
      .or_default()
      .push_back(reply);
  }

  /// Every served method and its params, in order, without pings.
  pub(crate) fn calls(&self) -> Vec<(String, Value)> {
    self.state().calls.clone()
  }
}

impl Drop for HerdrFixture {
  fn drop(&mut self) {
    self.state().stop_all();
  }
}

fn serve(stream: UnixStream, state: &Mutex<State>) {
  let Ok(mut writer) = stream.try_clone() else {
    return;
  };
  for line in BufReader::new(stream).lines().map_while(Result::ok) {
    let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let reply = {
      let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
      state.reply(method, &params)
    };
    let text = match reply {
      Ok(result) => json!({"id": id, "result": result}).to_string(),
      Err(Scripted::Error { code, message }) => {
        json!({"id": id, "error": {"code": code, "message": message}}).to_string()
      }
      Err(Scripted::Raw(text)) => text,
    };
    if writeln!(writer, "{text}").is_err() {
      return;
    }
  }
}
