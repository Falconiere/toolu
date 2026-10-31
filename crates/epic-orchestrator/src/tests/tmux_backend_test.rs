use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::{agent_scoped, execute, host_binary, parse_status};
use crate::herdr::Call;
use toolu_mux::status::AgentStatus;
use toolu_mux::tmux::{Tmux, TmuxError};

const PATH: &str = "/usr/bin:/bin:/opt/homebrew/bin";

/// A tmux client on a fresh socket, with a scratch repo for worktree calls.
struct World {
  _dir: tempfile::TempDir,
  env: Env,
  repo: std::path::PathBuf,
  socket: std::path::PathBuf,
}

impl World {
  fn new() -> World {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let socket = dir.path().join("tmux.sock");
    let env = Env::from_pairs([
      ("PATH", PATH),
      ("TOOLU_EPIC_TMUX_SOCKET", socket.to_str().unwrap()),
    ]);
    World {
      _dir: dir,
      env,
      repo,
      socket,
    }
  }

  /// A client on this world's socket.
  fn tmux(&self) -> Tmux {
    Tmux::new(&self.env, Some(self.socket.clone()))
  }

  /// A git repository with one committed file, bootstrapped in one go so a
  /// failing step names itself instead of leaving a half-built repo behind.
  fn git_repo(&self) {
    std::fs::write(self.repo.join("file.txt"), "hello\n").unwrap();
    let steps: &[&[&str]] = &[
      &["init", "-q"],
      &["config", "user.email", "test@example.com"],
      &["config", "user.name", "Test"],
      &["add", "file.txt"],
      &["commit", "-q", "-m", "init"],
    ];
    for step in steps {
      let mut argv = vec![
        "git".to_owned(),
        "-C".to_owned(),
        self.repo.to_string_lossy().into_owned(),
      ];
      argv.extend(step.iter().map(|arg| (*arg).to_owned()));
      let mut spec = Spec::new(argv);
      spec.env = Some(Env::from_pairs([("PATH", PATH)]));
      let output = run(&spec).unwrap();
      assert_eq!(output.exit_code, 0, "git {step:?}: {}", output.stderr);
    }
  }

  /// A `Call::Create` for `branch`, based on `main` in this world's repo.
  fn create_call(&self, branch: &str) -> Call {
    Call::Create {
      cwd: Some(self.repo.to_string_lossy().into_owned()),
      branch: Some(branch.to_owned()),
      base: Some("main".to_owned()),
      path: None,
      label: Some(branch.to_owned()),
      workspace: None,
    }
  }
}

impl Drop for World {
  /// Removing the scratch directory alone leaves the server running forever.
  fn drop(&mut self) {
    // No socket file: no server ever started; "no server": it already exited.
    if self.socket.exists()
      && let Err(err) = self.tmux().kill_server()
      && !err.message.contains("no server running")
    {
      eprintln!("tmux test server left running: {err:?}");
    }
  }
}

fn json(text: &str) -> Value {
  serde_json::from_str(text).unwrap_or_else(|err| panic!("not JSON: {text}: {err}"))
}

#[test]
fn create_and_remove_a_worktree_round_trip() {
  let world = World::new();
  world.git_repo();
  let create = world.create_call("feature");
  let text = execute(&create, &world.env);
  let reply = json(&text);
  let workspace_id = reply["workspace"]["workspace_id"]
    .as_str()
    .unwrap()
    .to_owned();
  assert!(
    reply["root_pane"]["pane_id"]
      .as_str()
      .unwrap()
      .starts_with('%')
  );

  let remove = Call::Remove {
    workspace_id,
    force: true,
  };
  let text = execute(&remove, &world.env);
  assert_eq!(json(&text), serde_json::json!({}));
}

#[test]
fn create_without_a_required_field_errors() {
  let world = World::new();
  let create = Call::Create {
    cwd: None,
    branch: Some("feature".to_owned()),
    base: Some("main".to_owned()),
    path: None,
    label: Some("feature".to_owned()),
    workspace: None,
  };
  let text = execute(&create, &world.env);
  let reply = json(&text);
  assert_eq!(reply["error"]["code"], "worktree_not_found");
}

#[test]
fn start_send_keys_and_read_round_trip() {
  let world = World::new();
  world.git_repo();
  let create = world.create_call("worker");
  let reply = json(&execute(&create, &world.env));
  let pane_id = reply["root_pane"]["pane_id"].as_str().unwrap().to_owned();

  let start = Call::Start {
    name: "worker".to_owned(),
    kind: "claude".to_owned(),
    pane_id: pane_id.clone(),
    args: vec!["started".to_owned()],
    timeout_ms: None,
  };
  // "claude" is unlikely to be on PATH in the test sandbox; the shell's own
  // "command not found" line still proves the argv reached the pane.
  execute(&start, &world.env);
  std::thread::sleep(std::time::Duration::from_millis(300));
  let read = Call::Read { target: pane_id };
  let text = execute(&read, &world.env);
  assert!(
    text.contains("claude") || text.contains("started"),
    "{text}"
  );
}

#[test]
fn a_missing_pane_is_reported_as_agent_not_found() {
  let world = World::new();
  // Anchor the server with a real session so the socket exists.
  let tmux = world.tmux();
  tmux.create_session("toolu-anchor", &world.repo).unwrap();
  let read = Call::Read {
    target: "%999999".to_owned(),
  };
  let text = execute(&read, &world.env);
  let reply = json(&text);
  assert_eq!(reply["error"]["code"], "agent_not_found");
}

#[test]
fn an_empty_workspace_list_is_well_formed() {
  let world = World::new();
  let text = execute(&Call::WorkspaceList, &world.env);
  assert_eq!(json(&text), serde_json::json!({"workspaces": []}));
}

#[test]
fn cursor_starts_the_cursor_agent_binary() {
  assert_eq!(host_binary("cursor"), "cursor-agent");
  assert_eq!(host_binary("claude"), "claude");
  assert_eq!(host_binary("not-a-host"), "not-a-host");
}

#[test]
fn pane_not_found_becomes_agent_not_found_for_agent_scoped_calls() {
  let err = TmuxError {
    code: "pane_not_found",
    message: "no pane".to_owned(),
  };
  assert_eq!(agent_scoped(err).code, "agent_not_found");
  let other = TmuxError {
    code: "workspace_not_found",
    message: "no session".to_owned(),
  };
  assert_eq!(agent_scoped(other).code, "workspace_not_found");
}

#[test]
fn worktree_list_pane_list_and_agent_list_report_the_open_session() {
  let world = World::new();
  world.git_repo();
  let create = world.create_call("feature");
  let created = json(&execute(&create, &world.env));
  let workspace_id = created["workspace"]["workspace_id"]
    .as_str()
    .unwrap()
    .to_owned();

  let list = Call::WorktreeList {
    cwd: world.repo.to_string_lossy().into_owned(),
  };
  let reply = json(&execute(&list, &world.env));
  let worktrees = reply["worktrees"].as_array().unwrap();
  let row = worktrees
    .iter()
    .find(|row| row["branch"] == "feature")
    .expect("feature worktree listed");
  assert_eq!(row["open_workspace_id"], workspace_id);

  let panes = Call::PaneList { workspace_id };
  let reply = json(&execute(&panes, &world.env));
  assert_eq!(
    reply["panes"][0]["pane_id"],
    created["root_pane"]["pane_id"]
  );

  let agents = json(&execute(&Call::AgentList, &world.env));
  let rows = agents["agents"].as_array().unwrap();
  assert!(rows.iter().any(|row| row["name"] == "feature"), "{rows:?}");

  let info = Call::PaneInfo {
    pane_id: created["root_pane"]["pane_id"].as_str().unwrap().to_owned(),
  };
  let reply = json(&execute(&info, &world.env));
  assert!(reply["process_info"]["shell_pid"].as_u64().unwrap() > 0);
}

#[test]
fn dropping_the_world_stops_its_server_and_shells() {
  let world = World::new();
  let tmux = world.tmux();
  tmux.create_session("toolu-drop", &world.repo).unwrap();
  let shell = tmux
    .pane_pid(&tmux.first_pane("toolu-drop").unwrap())
    .unwrap();
  drop(world);
  // The pane shell leads its own group; `alive` ignores an unreaped zombie.
  let alive = || toolu_runtime::process::group::alive(shell);
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
  while alive() && std::time::Instant::now() < deadline {
    std::thread::sleep(std::time::Duration::from_millis(50));
  }
  assert!(!alive(), "shell {shell} outlived its world");
}

#[test]
fn notify_shows_a_message_in_the_open_session() {
  let world = World::new();
  let tmux = world.tmux();
  tmux.create_session("toolu-notify", &world.repo).unwrap();
  let notify = Call::Notify {
    title: "attention".to_owned(),
    body: "needs a look".to_owned(),
  };
  assert_eq!(json(&execute(&notify, &world.env)), serde_json::json!({}));
}

#[test]
fn notify_without_any_session_is_a_quiet_success() {
  let world = World::new();
  let notify = Call::Notify {
    title: "attention".to_owned(),
    body: "needs a look".to_owned(),
  };
  assert_eq!(json(&execute(&notify, &world.env)), serde_json::json!({}));
}

#[test]
fn wait_reaches_done_once_the_shell_returns() {
  let world = World::new();
  world.git_repo();
  let create = world.create_call("waiter");
  let created = json(&execute(&create, &world.env));
  let pane_id = created["root_pane"]["pane_id"].as_str().unwrap().to_owned();
  let wait = Call::Wait {
    target: pane_id,
    until: vec!["done".to_owned()],
    timeout_ms: 5000,
  };
  assert_eq!(json(&execute(&wait, &world.env)), serde_json::json!({}));
}

#[test]
fn status_words_round_trip() {
  assert_eq!(parse_status("idle"), Some(AgentStatus::Idle));
  assert_eq!(parse_status("working"), Some(AgentStatus::Working));
  assert_eq!(parse_status("blocked"), Some(AgentStatus::Blocked));
  assert_eq!(parse_status("done"), Some(AgentStatus::Done));
  assert_eq!(parse_status("bogus"), None);
}
