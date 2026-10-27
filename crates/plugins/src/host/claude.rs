//! Claude Code's `plugin list --json`, an array of `{id, version, enabled?,
//! scope?}` where `id` is `<name>@<marketplace>`, and the result line of
//! `plugin update --json`. Only toolu's marketplace is kept from a listing, and a
//! name listed in several scopes (a project pin per worktree) is one plugin: the
//! user-scope row when there is one, else the first.

use serde_json::Value;

use crate::host::adapter::{Installed, MARKETPLACE};
use crate::host::run::{Ran, first_line};
use crate::step::{Status, Step};

/// The toolu plugins of a `claude plugin list --json` payload.
pub(crate) fn parse(stdout: &str) -> Result<Vec<Installed>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let entries = value.as_array().ok_or("expected an array of plugins")?;
  let mut kept: Vec<(Installed, bool)> = Vec::new();
  for item in entries {
    let Some(found) = entry(item)? else {
      continue;
    };
    let user = item.get("scope").and_then(Value::as_str) == Some("user");
    match kept.iter_mut().find(|(have, _)| have.name == found.name) {
      Some(slot) if user && !slot.1 => *slot = (found, user),
      Some(_kept) => {}
      None => kept.push((found, user)),
    }
  }
  Ok(kept.into_iter().map(|(entry, _)| entry).collect())
}

/// The row as a toolu plugin, or `None` when another marketplace installed it.
fn entry(item: &Value) -> Result<Option<Installed>, String> {
  let id = item
    .get("id")
    .and_then(Value::as_str)
    .ok_or("a plugin has no string \"id\"")?;
  let (name, marketplace) = match id.rsplit_once('@') {
    Some((name, marketplace)) if !name.is_empty() => (name, Some(marketplace)),
    _no_marketplace => (id, None),
  };
  if marketplace.is_some_and(|marketplace| marketplace != MARKETPLACE) {
    return Ok(None);
  }
  let version = item
    .get("version")
    .and_then(Value::as_str)
    .ok_or("a plugin has no string \"version\"")?;
  Ok(Some(Installed {
    name: name.to_owned(),
    version: version.to_owned(),
    enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
    source: None,
  }))
}

/// The step for `claude plugin update <name>@toolu --json`, read from the result
/// line Claude Code 2.1.311 prints (`fixtures/plugins/claude.json`); without a
/// readable line the exit code decides.
pub(crate) fn update_step(name: &str, ran: &Ran, argv: Vec<String>) -> Step {
  let result = ran.stdout.lines().rev().find_map(|line| {
    serde_json::from_str::<Value>(line.trim())
      .ok()
      .filter(Value::is_object)
  });
  let Some(result) = result else {
    return without_result(name, ran, argv);
  };
  let text = |key: &str| result.get(key).and_then(Value::as_str);
  // Claude Code says "unknown" when it cannot tell a version.
  let version = |key: &str| text(key).filter(|version| *version != "unknown");
  let message = text("message").map(str::to_owned);
  if !ran.ok() || text("outcome") == Some("failed") {
    let detail = message.unwrap_or_else(|| first_line(&ran.stderr, "update failed"));
    return Step::new(name, Status::Failed, detail, argv);
  }
  let new = version("newVersion");
  let refresh_failed = result.get("refreshFailed").and_then(Value::as_bool) == Some(true);
  match (text("updateOutcome"), new) {
    (Some("up_to_date"), Some(new)) if refresh_failed => Step::new(
      name,
      Status::Skew,
      format!("could not refresh the marketplace; still at {new}"),
      argv,
    ),
    (Some("up_to_date"), Some(new)) => {
      Step::new(name, Status::Current, format!("current at {new}"), argv)
    }
    (Some("up_to_date"), None) => Step::new(name, Status::Current, "current", argv),
    (Some("updated"), new) => Step::new(
      name,
      Status::Updated,
      moved(version("oldVersion"), new),
      argv,
    ),
    _skipped_or_unknown => Step::new(
      name,
      Status::Skew,
      message.unwrap_or_else(|| "skipped".to_owned()),
      argv,
    ),
  }
}

/// The step when Claude Code printed no result line: an update when it exited
/// 0, else a failure; a Claude Code without `plugin update --json` gets a hint.
fn without_result(name: &str, ran: &Ran, argv: Vec<String>) -> Step {
  if ran.ok() {
    return Step::new(name, Status::Updated, "updated", argv);
  }
  let mut detail = first_line(&ran.stderr, "update failed");
  if detail.contains("--json") {
    detail.push_str("; toolu needs Claude Code 2.1.311 or newer (`claude plugin update --json`)");
  }
  Step::new(name, Status::Failed, detail, argv)
}

/// `updated <old> → <new>`, or as much of it as Claude Code reported.
fn moved(old: Option<&str>, new: Option<&str>) -> String {
  match (old, new) {
    (Some(old), Some(new)) => format!("updated {old} → {new}"),
    (None, Some(new)) => format!("updated to {new}"),
    (_, None) => "updated".to_owned(),
  }
}

#[cfg(test)]
#[path = "tests/claude_test.rs"]
mod tests;
