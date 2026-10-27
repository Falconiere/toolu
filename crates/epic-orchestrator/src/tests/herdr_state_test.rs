//! What the herdr test server knows: workspaces with a shell pane each, live
//! agents, the call log and scripted replies.

use std::collections::{BTreeMap, VecDeque};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

/// A reply that replaces serving one call.
#[derive(Debug, Clone)]
pub(crate) enum Scripted {
  /// A herdr error object.
  Error { code: String, message: String },
  /// A raw reply line, such as non-JSON output.
  Raw(String),
}

/// One workspace and its single shell pane.
pub(crate) struct Workspace {
  pub id: String,
  pub label: String,
  pub path: PathBuf,
  pub repo: PathBuf,
  pub linked: bool,
  pub pane: String,
  pub shell: Child,
}

/// One live agent process.
pub(crate) struct Agent {
  pub name: String,
  pub kind: String,
  pub pane: String,
  pub cwd: PathBuf,
  pub status: String,
  /// herdr's `terminal_title_stripped`.
  pub title: String,
  pub prompts: Vec<String>,
  pub process: Child,
}

/// The server state.
pub(crate) struct State {
  pub protocol: u64,
  pub workspaces: Vec<Workspace>,
  pub agents: Vec<Agent>,
  pub calls: Vec<(String, Value)>,
  pub scripted: BTreeMap<String, VecDeque<Scripted>>,
  /// The `sh -c` script an agent process runs.
  pub agent_script: String,
  /// The text `agent.read` returns.
  pub tail: String,
  /// `/exit` requests agents ignore before one ends them.
  pub ignore_exits: u32,
  pub root: PathBuf,
  next: u64,
}

pub(crate) fn spawn(cwd: &Path, script: &str) -> Child {
  Command::new("sh")
    .args(["-c", script])
    .current_dir(cwd)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .process_group(0)
    .spawn()
    .expect("spawn")
}

/// Kill `child`'s process group and reap it. `killpg` itself: procps-ng's
/// `kill -KILL -<pgid>` can exit 0 on Linux without signalling the group,
/// which left `wait` blocked on the agent's `sleep`.
pub(crate) fn stop(child: &mut Child) {
  use toolu_runtime::process::group::{Signal, signal};
  let _killed = signal(child.id(), Signal::SIGKILL);
  let _reaped = child.wait();
}

pub(crate) fn text(params: &Value, key: &str) -> String {
  params
    .get(key)
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned()
}

pub(crate) fn error(code: &str, message: &str) -> Scripted {
  Scripted::Error {
    code: code.to_owned(),
    message: message.to_owned(),
  }
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, Scripted> {
  let out = Command::new("git")
    .arg("-C")
    .arg(cwd)
    .args(args)
    .output()
    .map_err(|err| error("git_failed", &err.to_string()))?;
  if out.status.success() {
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
  } else {
    Err(error(
      "git_failed",
      String::from_utf8_lossy(&out.stderr).trim(),
    ))
  }
}

impl State {
  pub(crate) fn new(root: &Path) -> Self {
    Self {
      protocol: 22,
      workspaces: Vec::new(),
      agents: Vec::new(),
      calls: Vec::new(),
      scripted: BTreeMap::new(),
      agent_script: "exec sleep 600".to_owned(),
      tail: String::new(),
      ignore_exits: 0,
      root: root.to_path_buf(),
      next: 0,
    }
  }

  /// Open a workspace on `path` with a fresh shell pane; its id.
  pub(crate) fn open(&mut self, repo: &Path, path: &Path, label: &str, linked: bool) -> String {
    self.next += 1;
    let id = format!("w{}", self.next);
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    self.workspaces.push(Workspace {
      pane: format!("{id}:p1"),
      shell: spawn(&path, "exec sleep 600"),
      id: id.clone(),
      label: label.to_owned(),
      repo: repo.to_path_buf(),
      linked,
      path,
    });
    id
  }

  /// Kill every shell and agent.
  pub(crate) fn stop_all(&mut self) {
    for agent in &mut self.agents {
      stop(&mut agent.process);
    }
    for workspace in &mut self.workspaces {
      stop(&mut workspace.shell);
    }
    self.agents.clear();
    self.workspaces.clear();
  }

  /// The reply to one request: a result, or a scripted or real error.
  pub(crate) fn reply(&mut self, method: &str, params: &Value) -> Result<Value, Scripted> {
    if method == "ping" {
      return Ok(json!({"type": "pong", "protocol": self.protocol}));
    }
    self.calls.push((method.to_owned(), params.clone()));
    self
      .agents
      .retain_mut(|agent| agent.process.try_wait().ok().flatten().is_none());
    if let Some(scripted) = self.scripted.get_mut(method).and_then(VecDeque::pop_front) {
      return Err(scripted);
    }
    match method {
      "workspace.list" => Ok(self.workspace_list()),
      "worktree.list" => self.worktree_list(params),
      "worktree.create" => self.create(params),
      "worktree.open" => self.open_existing(params),
      "worktree.remove" => self.remove(params),
      "pane.list" => self.pane_list(params),
      "pane.process_info" => self.process_info(params),
      "agent.list" => Ok(self.agent_list()),
      "agent.start" => self.start(params),
      "agent.prompt" | "agent.send_keys" => self.input(method, params),
      "agent.wait" => self
        .agent(&text(params, "target"))
        .map(|agent| json!({"agent": {"name": agent.name, "agent_status": agent.status}})),
      "agent.read" => Ok(json!({"read": {"text": self.tail}})),
      _ => Err(error("unknown_method", method)),
    }
  }

  pub(crate) fn workspace_json(workspace: &Workspace) -> Value {
    json!({
      "workspace_id": workspace.id,
      "label": workspace.label,
      "worktree": {
        "repo_root": workspace.repo.display().to_string(),
        "checkout_path": workspace.path.display().to_string(),
        "is_linked_worktree": workspace.linked,
      },
    })
  }

  fn workspace_list(&self) -> Value {
    json!({"workspaces": self.workspaces.iter().map(Self::workspace_json).collect::<Vec<_>>()})
  }

  fn worktree_list(&self, params: &Value) -> Result<Value, Scripted> {
    let listed = git(
      Path::new(&text(params, "cwd")),
      &["worktree", "list", "--porcelain"],
    )?;
    let rows: Vec<Value> = listed
      .split("\n\n")
      .filter_map(|block| {
        let path = block
          .lines()
          .find_map(|line| line.strip_prefix("worktree "))?;
        let branch = block
          .lines()
          .find_map(|line| line.strip_prefix("branch refs/heads/"))
          .unwrap_or("");
        let open = self
          .workspaces
          .iter()
          .find(|ws| ws.path == Path::new(path))
          .map(|ws| ws.id.clone());
        Some(json!({"path": path, "branch": branch, "open_workspace_id": open}))
      })
      .collect();
    Ok(json!({"worktrees": rows}))
  }

  fn opened(&self, id: &str) -> Value {
    let workspace = self.workspaces.iter().find(|ws| ws.id == id);
    workspace.map_or(Value::Null, |ws| {
      json!({
        "workspace": Self::workspace_json(ws),
        "root_pane": {"pane_id": ws.pane, "workspace_id": ws.id},
        "worktree": {"path": ws.path.display().to_string()},
      })
    })
  }

  fn create(&mut self, params: &Value) -> Result<Value, Scripted> {
    let cwd = PathBuf::from(text(params, "cwd"));
    let branch = text(params, "branch");
    let path = self.root.join("worktrees").join(branch.replace('/', "-"));
    let target = path.display().to_string();
    git(
      &cwd,
      &[
        "worktree",
        "add",
        "-q",
        "-b",
        &branch,
        &target,
        &text(params, "base"),
      ],
    )?;
    let id = self.open(&cwd, &path, &text(params, "label"), true);
    Ok(self.opened(&id))
  }

  fn open_existing(&mut self, params: &Value) -> Result<Value, Scripted> {
    let path = PathBuf::from(text(params, "path"));
    if !path.exists() {
      return Err(error("worktree_not_found", &text(params, "path")));
    }
    let id = self.open(
      Path::new(&text(params, "cwd")),
      &path,
      &text(params, "label"),
      true,
    );
    Ok(self.opened(&id))
  }

  fn remove(&mut self, params: &Value) -> Result<Value, Scripted> {
    let id = text(params, "workspace_id");
    let index = self
      .workspaces
      .iter()
      .position(|ws| ws.id == id)
      .ok_or_else(|| error("workspace_not_found", &id))?;
    let mut workspace = self.workspaces.remove(index);
    stop(&mut workspace.shell);
    let pane = workspace.pane.clone();
    for agent in self.agents.iter_mut().filter(|agent| agent.pane == pane) {
      stop(&mut agent.process);
    }
    self.agents.retain(|agent| agent.pane != pane);
    let mut args = vec!["worktree", "remove"];
    if params.get("force").and_then(Value::as_bool) == Some(true) {
      args.push("--force");
    }
    let path = workspace.path.display().to_string();
    args.push(&path);
    git(&workspace.repo, &args)?;
    Ok(json!({"workspace_id": id}))
  }
}
