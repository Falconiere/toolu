//! The herdr test server's panes and agents: process info, the agent list,
//! starts, prompts and keys.

use serde_json::{Value, json};

use crate::herdr_state::{Agent, Scripted, State, Workspace, error, spawn, stop, text};

impl State {
  pub(crate) fn pane(&self, pane: &str) -> Result<&Workspace, Scripted> {
    self
      .workspaces
      .iter()
      .find(|ws| ws.pane == pane)
      .ok_or_else(|| error("pane_not_found", pane))
  }

  pub(crate) fn pane_list(&self, params: &Value) -> Result<Value, Scripted> {
    let id = text(params, "workspace_id");
    let ws = self
      .workspaces
      .iter()
      .find(|ws| ws.id == id)
      .ok_or_else(|| error("workspace_not_found", &id))?;
    Ok(
      json!({"panes": [{"pane_id": ws.pane, "workspace_id": ws.id, "cwd": ws.path.display().to_string()}]}),
    )
  }

  pub(crate) fn process_info(&self, params: &Value) -> Result<Value, Scripted> {
    let ws = self.pane(&text(params, "pane_id"))?;
    let shell = ws.shell.id();
    Ok(
      json!({"process_info": {"pane_id": ws.pane, "shell_pid": shell, "foreground_process_group_id": shell}}),
    )
  }

  pub(crate) fn agent_list(&self) -> Value {
    let agents: Vec<Value> = self
      .agents
      .iter()
      .map(|agent| {
        json!({
          "name": agent.name, "agent": agent.kind, "pane_id": agent.pane,
          "cwd": agent.cwd.display().to_string(), "agent_status": agent.status,
          "terminal_title_stripped": agent.title,
          "workspace_id": agent.pane.split(':').next().unwrap_or(""), "terminal_id": agent.pane,
        })
      })
      .collect();
    json!({"agents": agents})
  }

  pub(crate) fn agent(&self, name: &str) -> Result<&Agent, Scripted> {
    self
      .agents
      .iter()
      .find(|agent| agent.name == name)
      .ok_or_else(|| error("agent_not_found", name))
  }

  pub(crate) fn start(&mut self, params: &Value) -> Result<Value, Scripted> {
    let name = text(params, "name");
    if self.agent(&name).is_ok() {
      return Err(error("agent_exists", &name));
    }
    let pane = text(params, "pane_id");
    let cwd = self.pane(&pane)?.path.clone();
    self.agents.push(Agent {
      process: spawn(&cwd, &self.agent_script),
      kind: text(params, "kind"),
      status: "idle".to_owned(),
      title: name.clone(),
      prompts: Vec::new(),
      name: name.clone(),
      pane,
      cwd,
    });
    Ok(json!({"agent": {"name": name}}))
  }

  /// A prompt or keys; `/exit` ends the agent unless it ignores exits.
  pub(crate) fn input(&mut self, method: &str, params: &Value) -> Result<Value, Scripted> {
    let name = text(params, "target");
    let words: Vec<String> = if method == "agent.prompt" {
      vec![text(params, "text")]
    } else {
      let keys = params
        .get("keys")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
      keys
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
    };
    let index = self
      .agents
      .iter()
      .position(|agent| agent.name == name)
      .ok_or_else(|| error("agent_not_found", &name))?;
    let agent = self
      .agents
      .get_mut(index)
      .ok_or_else(|| error("agent_not_found", &name))?;
    agent.prompts.extend(words.iter().cloned());
    if method == "agent.prompt" {
      agent.status = "working".to_owned();
    }
    let exits = words.iter().any(|word| word == "/exit");
    if exits && self.ignore_exits > 0 {
      self.ignore_exits -= 1;
    } else if exits {
      stop(&mut agent.process);
      self.agents.remove(index);
    }
    Ok(json!({}))
  }
}
