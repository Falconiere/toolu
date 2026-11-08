//! Capture a Grok Build session for a worktree. Grok writes each session at
//! start to `<home>/sessions/<encoded cwd>/<uuid>/summary.json`, the cwd
//! percent-encoded with the RFC 3986 unreserved bytes kept. The summary names
//! the session (`info.id`) and its cwd (`info.cwd`); a subagent's own session
//! sits in the same directory with `session_kind` `subagent`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::Value;
use toolu_runtime::env::Env;

use super::session::{MAX_FILES, fresh_head, one, valid_id, vanished};

/// Grok's home: `$GROK_HOME`, else `~/.grok`.
pub(crate) fn grok_home(env: &Env) -> PathBuf {
  env
    .get("GROK_HOME")
    .filter(|dir| !dir.trim().is_empty())
    .map_or_else(|| env.home().join(".grok"), PathBuf::from)
}

/// The directory Grok keeps the sessions of `worktree` in.
pub(crate) fn group(env: &Env, worktree: &str) -> PathBuf {
  // shortcut: Grok names a group whose encoded name exceeds 255 bytes by a
  // slug and hash (with a `.cwd` file), which this misses; read `.cwd` files
  // if a worktree path ever gets that long.
  let mut name = String::with_capacity(worktree.len());
  for byte in worktree.bytes() {
    if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
      name.push(char::from(byte));
    } else {
      let _written = write!(name, "%{byte:02X}");
    }
  }
  grok_home(env).join("sessions").join(name)
}

/// The session a summary names when it is a top-level session of `worktree`.
fn top_level(text: &str, worktree: &str) -> Option<String> {
  let summary: Value = serde_json::from_str(text).ok()?;
  let subagent = summary.get("session_kind").and_then(Value::as_str) == Some("subagent");
  let cwd = summary.pointer("/info/cwd").and_then(Value::as_str)?;
  let id = summary.pointer("/info/id").and_then(Value::as_str)?;
  (!subagent && cwd == worktree).then(|| id.to_owned())
}

/// The one top-level Grok session written for `worktree` since `since`
/// (epoch ms) whose summary names its own directory.
///
/// # Errors
/// The session directory cannot be read for a reason other than vanishing.
pub(super) fn capture(worktree: &str, since: u64, env: &Env) -> Result<Option<String>, String> {
  let root = group(env, worktree);
  let entries = match std::fs::read_dir(&root) {
    Ok(entries) => entries,
    Err(err) if vanished(&err) => return Ok(None),
    Err(err) => return Err(format!("{}: {err}", root.display())),
  };
  let names: Vec<String> = entries
    .filter_map(Result::ok)
    .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
    .filter(|name| valid_id(name))
    .collect();
  if names.len() > MAX_FILES {
    return Ok(None);
  }
  let mut found = BTreeSet::new();
  for name in names {
    let Some(text) = fresh_head(&root.join(&name).join("summary.json"), since)? else {
      continue;
    };
    if top_level(&text, worktree).as_deref() == Some(name.as_str()) {
      found.insert(name);
    }
  }
  Ok(one(found))
}

#[cfg(test)]
#[path = "tests/grok_session_test.rs"]
mod tests;
