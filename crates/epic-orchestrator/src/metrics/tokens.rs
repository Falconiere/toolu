//! Token use per worker, read from the host's own session records (#451):
//! Claude Code transcripts (with their subagent transcripts) and Codex
//! rollouts. `opencode` keeps usage in a database and `cursor` in no
//! readable store, so their rows say `unavailable`; nothing is estimated.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

/// One worker as its issue record names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Agent {
  /// The issue key.
  pub key: String,
  /// `claude`, `codex`, `opencode` or `cursor`.
  pub host: String,
  /// The host's session id, when the launch captured it.
  pub session: Option<String>,
  /// The worker's worktree.
  pub worktree: String,
  /// Launches of the issue; only the last session id is kept.
  pub launches: u64,
}

const CLAUDE_FIELDS: [&str; 4] = [
  "input_tokens",
  "output_tokens",
  "cache_read_input_tokens",
  "cache_creation_input_tokens",
];

/// The agents of the issue records in `<state>/issues/`.
///
/// # Errors
/// The directory or a record cannot be read.
pub(crate) fn agents(state: &Path) -> Result<Vec<Agent>, String> {
  let dir = state.join("issues");
  let entries = std::fs::read_dir(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  let mut agents = Vec::new();
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.extension().is_none_or(|ext| ext != "json") {
      continue;
    }
    let text =
      std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let record: Value =
      serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    let field = |name: &str| record.get(name).and_then(Value::as_str).map(str::to_owned);
    agents.push(Agent {
      key: field("key").unwrap_or_default(),
      host: field("kind").unwrap_or_default(),
      session: field("session_id"),
      worktree: field("worktree").unwrap_or_default(),
      launches: record.get("launches").and_then(Value::as_u64).unwrap_or(1),
    });
  }
  agents.sort_by(|a, b| a.key.cmp(&b.key));
  Ok(agents)
}

/// The token row of `agent`.
pub(crate) fn row(agent: &Agent, env: &Env) -> Value {
  let found = match (agent.host.as_str(), agent.session.as_deref()) {
    (_, None) => Err("no session id was captured at launch"),
    ("claude", Some(session)) => claude(agent, session, env),
    ("codex", Some(session)) => codex(session, env),
    ("opencode", Some(_)) => Err("OpenCode keeps usage in its database, which toolu does not read"),
    (_, Some(_)) => Err("this host records no token use toolu can read"),
  };
  match found {
    Ok((source, tokens)) => json!({
      "key": agent.key, "host": agent.host, "source": source,
      "complete": agent.launches <= 1, "tokens": tokens,
    }),
    Err(reason) => json!({
      "key": agent.key, "host": agent.host, "status": "unavailable", "reason": reason,
    }),
  }
}

fn claude(agent: &Agent, session: &str, env: &Env) -> Result<(&'static str, Value), &'static str> {
  let home = env
    .get("CLAUDE_CONFIG_DIR")
    .map_or_else(|| env.home().join(".claude"), PathBuf::from);
  let slug: String = agent
    .worktree
    .chars()
    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
    .collect();
  let project = home.join("projects").join(slug);
  let main = project.join(format!("{session}.jsonl"));
  let text = std::fs::read_to_string(&main).map_err(|_err| "no transcript for the session")?;
  let mut texts = vec![text];
  if let Ok(entries) = std::fs::read_dir(project.join(session).join("subagents")) {
    let paths = entries.filter_map(|entry| entry.ok().map(|entry| entry.path()));
    texts.extend(paths.filter_map(|path| std::fs::read_to_string(path).ok()));
  }
  let mut last: BTreeMap<String, Value> = BTreeMap::new();
  for line in texts.iter().flat_map(|text| text.lines()) {
    let Ok(entry) = serde_json::from_str::<Value>(line) else {
      continue;
    };
    let usage = entry.pointer("/message/usage").cloned();
    if let (Some("assistant"), Some(id), Some(usage)) = (
      entry.get("type").and_then(Value::as_str),
      entry.pointer("/message/id").and_then(Value::as_str),
      usage,
    ) {
      last.insert(id.to_owned(), usage);
    }
  }
  let sum = |field: &str| -> u64 {
    last
      .values()
      .map(|usage| usage.get(field).and_then(Value::as_u64).unwrap_or(0))
      .sum()
  };
  let tokens: serde_json::Map<String, Value> = CLAUDE_FIELDS
    .iter()
    .map(|field| ((*field).to_owned(), json!(sum(field))))
    .collect();
  Ok(("claude-code transcript", Value::Object(tokens)))
}

fn codex(session: &str, env: &Env) -> Result<(&'static str, Value), &'static str> {
  let home = env
    .get("CODEX_HOME")
    .map_or_else(|| env.home().join(".codex"), PathBuf::from);
  let suffix = format!("{session}.jsonl");
  let path = find_file(&home.join("sessions"), &suffix, 4).ok_or("no rollout for the session")?;
  let text = std::fs::read_to_string(path).map_err(|_err| "the rollout cannot be read")?;
  text
    .lines()
    .rev()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    .find(|entry| entry.pointer("/payload/type").and_then(Value::as_str) == Some("token_count"))
    .and_then(|entry| entry.pointer("/payload/info/total_token_usage").cloned())
    .map(|tokens| ("codex rollout", tokens))
    .ok_or("the rollout has no token count")
}

/// The first file under `dir`, at most `depth` levels down, whose name ends in `suffix`.
fn find_file(dir: &Path, suffix: &str, depth: u32) -> Option<PathBuf> {
  let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
    .ok()?
    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
    .collect();
  entries.sort();
  for path in entries {
    let name = path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned());
    if path.is_file() && name.is_some_and(|name| name.ends_with(suffix)) {
      return Some(path);
    }
    if path.is_dir()
      && depth > 0
      && let Some(found) = find_file(&path, suffix, depth - 1)
    {
      return Some(found);
    }
  }
  None
}

#[cfg(test)]
#[path = "tests/tokens_test.rs"]
mod tests;
