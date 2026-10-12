//! One attention notice: the herdr title and body, and the outgoing POST body.

use std::path::Path;

use serde_json::{Value, json};

use crate::disk::read_value;
use crate::model::{Attention, World};

/// The longest herdr notification body, in Unicode scalar values.
const BODY_CAP: usize = 200;

/// What the notifier thread announces for one raise or reminder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
  /// The item token, for the journal.
  pub token: String,
  /// The item's issue key, for the journal.
  pub key: String,
  /// herdr title, `toolu epic: <key> <kind>`.
  pub title: String,
  /// herdr body: the note, capped.
  pub body: String,
  /// The POST body.
  pub payload: Value,
}

/// The notice for item `token`, or `None` once it has closed.
pub(crate) fn build(
  world: &World,
  token: &str,
  reminder: bool,
  dashboard: Option<&str>,
) -> Option<Notice> {
  let item = world.attention.iter().find(|item| item.token == token)?;
  let issue = world
    .issues
    .get(&item.key)
    .and_then(|issue| snapshot(Path::new(&issue.state_dir), &item.key));
  let title = format!("toolu epic: {} {}", item.key, item.kind);
  Some(Notice {
    token: item.token.clone(),
    key: item.key.clone(),
    body: item.note.chars().take(BODY_CAP).collect(),
    payload: payload(item, world.now_ms, reminder, issue.as_ref(), dashboard),
    title,
  })
}

/// `{title, url}` from `<state_dir>/issues/<key>.json`, when it has either.
fn snapshot(state_dir: &Path, key: &str) -> Option<Value> {
  let path = state_dir.join("issues").join(format!("{key}.json"));
  let value = read_value(&path).ok()?;
  let field = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
  let (title, url) = (field("title"), field("url"));
  (title.is_some() || url.is_some()).then(|| json!({"title": title, "url": url}))
}

fn payload(
  item: &Attention,
  now_ms: u64,
  reminder: bool,
  issue: Option<&Value>,
  dashboard: Option<&str>,
) -> Value {
  let url = issue
    .and_then(|issue| issue.get("url"))
    .and_then(Value::as_str);
  let again = if reminder { " (reminder)" } else { "" };
  let head = format!(
    "toolu epic: {} {}{again}: {}",
    item.key, item.kind, item.note
  );
  let text = [Some(head.as_str()), url, dashboard]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
  json!({
    "text": text,
    "kind": item.kind,
    "key": item.key,
    "epic": item.epic,
    "state": item.state.name(),
    "note": item.note,
    "age_seconds": now_ms.saturating_sub(item.raised_ms) / 1000,
    "reminder": reminder,
    "issue": issue,
    "dashboard": dashboard,
  })
}

/// `http://<host>:<port>/` for the status listener, with an unspecified bind
/// address shown as loopback.
pub(crate) fn dashboard(addr: std::net::SocketAddr) -> String {
  let host = if addr.ip().is_unspecified() {
    "127.0.0.1".to_owned()
  } else if addr.is_ipv6() {
    format!("[{}]", addr.ip())
  } else {
    addr.ip().to_string()
  };
  format!("http://{host}:{}/", addr.port())
}

#[cfg(test)]
#[path = "tests/notify_notice_test.rs"]
mod tests;
