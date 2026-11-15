//! Token use per worker, read from the host's own session records (#451):
//! Claude Code transcripts (with their subagent transcripts), Codex rollouts,
//! Muse session logs (`muse_tokens`) and Grok usage records. `opencode` keeps
//! usage in a database and `cursor` in no readable store, so their rows say
//! `unavailable`; nothing is estimated.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

/// One worker as its issue record names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Agent {
  /// The issue key.
  pub key: String,
  /// `claude`, `codex`, `muse`, `grok`, `opencode` or `cursor`.
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

const GROK_FIELDS: [&str; 6] = [
  "inputTokens",
  "outputTokens",
  "cachedReadTokens",
  "cacheCreationTokens",
  "reasoningTokens",
  "totalTokens",
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
  read(agent, env, false)
}

/// A durable snapshot marks partial native records as incomplete.
pub(super) fn snapshot(agent: &Agent, env: &Env) -> Value {
  read(agent, env, true)
}

fn read(agent: &Agent, env: &Env, strict: bool) -> Value {
  let mut partial = false;
  let found = match (agent.host.as_str(), agent.session.as_deref()) {
    (_, None) => Err("no session id was captured at launch"),
    ("claude", Some(session)) => claude(agent, session, env, strict, &mut partial),
    ("codex", Some(session)) => codex(session, env, strict, &mut partial),
    ("muse", Some(session)) if strict => super::muse_tokens::read(session, env, true, &mut partial),
    ("muse", Some(session)) => super::muse_tokens::muse(session, env),
    ("grok", Some(session)) => grok(agent, session, env, strict),
    ("opencode", Some(_)) => Err("OpenCode keeps usage in its database, which toolu does not read"),
    (_, Some(_)) => Err("this host records no token use toolu can read"),
  };
  match found {
    Ok((source, tokens)) if strict && partial => json!({
      "key": agent.key, "host": agent.host, "source": source, "complete": false,
      "tokens": tokens, "status": "partial", "reason": "session log contains malformed JSON",
    }),
    Ok((source, tokens)) => json!({
      "key": agent.key, "host": agent.host, "source": source,
      "complete": agent.launches <= 1, "tokens": tokens,
    }),
    Err(reason) => json!({
      "key": agent.key, "host": agent.host, "status": "unavailable", "reason": reason,
    }),
  }
}

fn claude(
  agent: &Agent,
  session: &str,
  env: &Env,
  strict: bool,
  partial: &mut bool,
) -> Result<(&'static str, Value), &'static str> {
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
  let children = project.join(session).join("subagents");
  match std::fs::read_dir(children) {
    Ok(entries) => {
      for entry in entries {
        let log = entry
          .map_err(|_err| "cannot list subagent transcripts")
          .and_then(|entry| {
            std::fs::read_to_string(entry.path())
              .map_err(|_err| "a subagent transcript cannot be read")
          });
        match log {
          Ok(log) => texts.push(log),
          Err(reason) if strict => return Err(reason),
          Err(_) => {}
        }
      }
    }
    Err(err) if strict && err.kind() != std::io::ErrorKind::NotFound => {
      return Err("cannot list subagent transcripts");
    }
    Err(_) => {}
  }
  let mut last = BTreeMap::new();
  for text in texts {
    claude_usage(&text, &mut last, strict, partial)?;
  }
  Ok((
    "claude-code transcript",
    field_totals(&last, &CLAUDE_FIELDS),
  ))
}

fn claude_usage(
  text: &str,
  into: &mut BTreeMap<String, Value>,
  strict: bool,
  partial: &mut bool,
) -> Result<(), &'static str> {
  let mut seen = false;
  for line in text.lines() {
    let Some(entry) = parse_line(line, partial) else {
      continue;
    };
    if let (Some("assistant"), Some(id), Some(usage)) = (
      entry.get("type").and_then(Value::as_str),
      entry.pointer("/message/id").and_then(Value::as_str),
      entry.pointer("/message/usage"),
    ) {
      if strict && !valid_counts(usage, &CLAUDE_FIELDS) {
        return Err("transcript usage counters cannot be read");
      }
      into.insert(id.to_owned(), usage.clone());
      seen = true;
    }
  }
  if strict && !seen {
    return Err("transcript has no token usage");
  }
  Ok(())
}

/// Parse a log line and remember malformed JSON for snapshot coverage.
pub(super) fn parse_line(line: &str, partial: &mut bool) -> Option<Value> {
  if line.trim().is_empty() {
    return None;
  }
  if let Ok(entry) = serde_json::from_str(line) {
    Some(entry)
  } else {
    *partial = true;
    None
  }
}

/// Input and output must be recorded; optional known counters must be unsigned.
pub(super) fn valid_counts(value: &Value, fields: &[&str]) -> bool {
  fields
    .iter()
    .take(2)
    .all(|field| value.get(field).and_then(Value::as_u64).is_some())
    && fields
      .iter()
      .all(|field| value.get(field).is_none_or(|v| v.as_u64().is_some()))
}

/// Each of `fields` summed over every usage object, saturating; a missing
/// field counts as zero.
pub(super) fn field_totals(usages: &BTreeMap<String, Value>, fields: &[&str]) -> Value {
  let sum = |field: &str| -> u64 {
    usages
      .values()
      .map(|usage| usage.get(field).and_then(Value::as_u64).unwrap_or(0))
      .fold(0, u64::saturating_add)
  };
  let tokens: serde_json::Map<String, Value> = fields
    .iter()
    .map(|field| ((*field).to_owned(), json!(sum(field))))
    .collect();
  Value::Object(tokens)
}

/// Grok's per-session totals, which already include its subagents' calls.
fn grok(
  agent: &Agent,
  session: &str,
  env: &Env,
  strict: bool,
) -> Result<(&'static str, Value), &'static str> {
  let path = crate::launch::grok_session::group(env, &agent.worktree)
    .join(session)
    .join("usage.json");
  let text = std::fs::read_to_string(path).map_err(|_err| "no usage record for the session")?;
  let usage = serde_json::from_str::<Value>(&text)
    .map_err(|_err| "the usage record cannot be read")?
    .get("session")
    .cloned()
    .filter(Value::is_object)
    .ok_or("the usage record cannot be read")?;
  if strict && !valid_counts(&usage, &GROK_FIELDS) {
    return Err("the usage record has no valid token counts");
  }
  let totals = BTreeMap::from([(session.to_owned(), usage)]);
  Ok(("grok usage record", field_totals(&totals, &GROK_FIELDS)))
}

fn codex(
  session: &str,
  env: &Env,
  strict: bool,
  partial: &mut bool,
) -> Result<(&'static str, Value), &'static str> {
  let home = env
    .get("CODEX_HOME")
    .map_or_else(|| env.home().join(".codex"), PathBuf::from);
  let suffix = format!("{session}.jsonl");
  let path = find_file(&home.join("sessions"), &suffix, 4).ok_or("no rollout for the session")?;
  let text = std::fs::read_to_string(path).map_err(|_err| "the rollout cannot be read")?;
  let mut latest = None;
  for line in text.lines().rev() {
    let Some(entry) = parse_line(line, partial) else {
      continue;
    };
    if latest.is_none()
      && entry.pointer("/payload/type").and_then(Value::as_str) == Some("token_count")
    {
      latest = Some(entry.pointer("/payload/info/total_token_usage").cloned());
    }
    if !strict && latest.is_some() {
      break;
    }
  }
  let tokens = latest.flatten().ok_or("the rollout has no token count")?;
  if strict
    && !valid_counts(
      &tokens,
      &[
        "input_tokens",
        "output_tokens",
        "cached_input_tokens",
        "reasoning_output_tokens",
        "total_tokens",
      ],
    )
  {
    return Err("the rollout has no valid token counts");
  }
  Ok(("codex rollout", tokens))
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
