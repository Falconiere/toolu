//! Opt-in, bounded lifecycle notices for peer sessions.
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use toolu_mux::relay::{config, peers::client, write_state};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock_strict};

fn binding(env: &Env, host: Host, payload: &str) -> Option<PathBuf> {
  let value: Value = serde_json::from_str(payload).ok()?;
  let id = value.get("session_id")?.as_str()?;
  if id.is_empty()
    || id.len() > 128
    || !id
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
  {
    return None;
  }
  Some(
    config::root(env)
      .join("peers/bindings")
      .join(format!("{}-{id}", host.name())),
  )
}
fn session(env: &Env, host: Host, payload: &str, startup: bool) -> Result<String, String> {
  let path = binding(env, host, payload).ok_or("host session ID unavailable")?;
  let parent = path.parent().ok_or("invalid binding path")?;
  std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  let options = LockOptions {
    timeout: Duration::from_millis(100),
    stale: Duration::MAX,
  };
  with_lock_strict(&path, options, || {
    if !startup {
      return std::fs::read_to_string(&path).map_err(|e| e.to_string());
    }
    let input = client::Join {
      group: env.get("TOOLU_PEERS_GROUP").ok_or("group missing")?,
      machine: env.get("TOOLU_PEERS_MACHINE").ok_or("machine missing")?,
      host: host.name(),
      key_file: env.get("TOOLU_PEERS_KEY_FILE").ok_or("key file missing")?,
    };
    let id = client::prepare(env, &input)?;
    write_state(&path, id.as_bytes())?;
    let _ = client::wake::enroll(env, &id, input.machine, input.host);
    Ok(id)
  })?
}
/// Return a bounded notice without displaying peer-supplied content.
pub(crate) fn notice(env: &Env, host: Host, payload: &str, event: &str) -> Option<String> {
  env.get("TOOLU_PEERS_GROUP")?;
  env.get("TOOLU_PEERS_MACHINE")?;
  env.get("TOOLU_PEERS_KEY_FILE")?;
  let startup = event == "startup";
  let Ok(id) = session(env, host, payload, startup) else {
    return Some("Peers unavailable: use toolu peers join --group ID --machine NAME --agent HOST --key-file FILE.".into());
  };
  let timeout = if startup {
    Duration::from_secs(1)
  } else {
    Duration::from_secs(2)
  };
  let state = client::register(env, &id, timeout).and_then(|_| {
    if startup {
      return Ok(None);
    }
    let _ = client::sync(env, &id, None, 1, timeout);
    client::request(env, &id, "inbox", json!({"unread":true}), timeout)
      .map(|value| value.get("unread").and_then(Value::as_u64))
  });
  let count = match state {
    Ok(Some(n)) => format!("{n} unread"),
    Ok(None) => "joined".to_owned(),
    Err(_) => "unavailable; registration saved".to_owned(),
  };
  Some(format!(
    "Peers session {id}: {count}. Use toolu peers --session {id} list, inbox, reply KEY --text TEXT, or ack KEY."
  ))
}

#[cfg(test)]
#[path = "tests/notice_test.rs"]
mod tests;
