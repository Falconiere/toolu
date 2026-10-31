//! `tick`: one babysit check. It holds the slot lock, checks the previous
//! state, collects the pull request (or reads a recorded snapshot), reduces,
//! and writes the snapshot, state and result files. A failed read stamps
//! `pr.lastError` on the previous state and changes nothing else.

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};
use toolu_engine::babysit::GraphQlUsage;
use toolu_github::{Client, Config};
use toolu_runtime::env::Env;
use toolu_state::time::parse_iso;

use crate::collect;
use crate::error::{Failure, Result};
use crate::lock::SlotLock;
use crate::reduce::{Paths, reduce};
use crate::schedule;
use crate::state::{sibling, write_json, write_text};

/// Where the snapshot comes from.
pub(crate) enum Source<'a> {
  /// A recorded snapshot file (`--snapshot-in`).
  File(&'a Path),
  /// GitHub, through a client built from this configuration.
  GitHub {
    /// The client's API, retries and deadline.
    config: Config,
    /// Items per page.
    page_size: u64,
    /// Keeps the built client for later ticks; `None` builds one per tick.
    client: Option<&'a Mutex<Option<Client>>>,
  },
}

/// One check of one pull request.
pub(crate) struct Request<'a> {
  /// `owner/repo`.
  pub(crate) repo: &'a str,
  /// The pull request number.
  pub(crate) number: u64,
  /// The state file.
  pub(crate) state_file: &'a Path,
  /// The snapshot file; `<stem>.snapshot.json` when `None`.
  pub(crate) snapshot_out: Option<&'a Path>,
  /// The check's time, `YYYY-MM-DDTHH:MM:SSZ`.
  pub(crate) now: &'a str,
  /// Whether this check moves the schedule (`wait` and the engine do).
  pub(crate) scheduled: bool,
}

/// A tick's result and what its GraphQL reads cost.
pub(crate) struct Ticked {
  /// The version-1 result.
  pub(crate) result: Value,
  /// The GraphQL budget a collection spent; `None` for a recorded snapshot.
  pub(crate) graphql: Option<GraphQlUsage>,
}

/// Run one check.
///
/// # Errors
/// `locked`, `state_malformed`, `slot_mismatch`, a collection failure, an
/// invalid snapshot, or a write failure.
pub(crate) fn tick(request: &Request<'_>, source: Source<'_>, env: &Env) -> Result<Ticked> {
  let now = parse_iso(request.now)
    .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
    .map(|since| since.as_secs())
    .ok_or_else(|| Failure::usage("--now must be YYYY-MM-DDTHH:MM:SSZ"))?;
  let _lock = SlotLock::acquire(request.state_file, env)?;
  let previous = previous(request)?;
  let collected = matches!(source, Source::GitHub { .. });
  let read = match snapshot(request, source, env) {
    Ok(read) => valid(&read.0).map(|()| read),
    Err(failure) if !collected => return Err(failure),
    Err(failure) => Err(failure),
  };
  let (snapshot, raw, graphql) = match read {
    Ok(read) => read,
    Err(failure) => {
      stamp(request, previous, &failure);
      return Err(failure);
    }
  };
  let snapshot_out = request.snapshot_out.map_or_else(
    || sibling(request.state_file, ".snapshot.json"),
    Path::to_path_buf,
  );
  let result_out = sibling(request.state_file, ".result.json");
  let shown = |path: &Path| path.display().to_string();
  let (state_path, snapshot_path, result_path) = (
    shown(request.state_file),
    shown(&snapshot_out),
    shown(&result_out),
  );
  let paths = Paths {
    state: &state_path,
    snapshot: &snapshot_path,
    result: &result_path,
  };
  let mut reduced = reduce(&snapshot, previous.as_ref(), request.now, &paths);
  schedule::apply(&mut reduced, now, request.scheduled);
  let raw = raw.unwrap_or_else(|| format!("{snapshot}\n"));
  write_text(&snapshot_out, &raw, "snapshot_out")?;
  write_json(request.state_file, &reduced.state, "state")?;
  write_json(&result_out, &reduced.result, "result")?;
  Ok(Ticked {
    result: reduced.result,
    graphql,
  })
}

fn previous(request: &Request<'_>) -> Result<Option<Value>> {
  let path = request.state_file;
  let malformed =
    |message: String| Failure::new("state_malformed", message).with("source", "state");
  let file = path.display();
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(err) => {
      return Err(malformed(format!(
        "state file cannot be read: {file}: {err}"
      )));
    }
  };
  let value: Value = serde_json::from_str(&text)
    .ok()
    .ok_or_else(|| malformed(format!("state file is not valid JSON: {file}")))?;
  if value.get("version").and_then(Value::as_u64) != Some(2) {
    let version = value.get("version").cloned().unwrap_or(Value::Null);
    return Err(malformed(format!("state file is not version 2: {file}")).with("version", version));
  }
  let (repo, number) = (value.get("repo"), value.get("number"));
  if repo.and_then(Value::as_str) != Some(request.repo)
    || number.and_then(Value::as_u64) != Some(request.number)
  {
    let (repo, number) = (repo.cloned(), number.cloned());
    let owner = format!("{}#{}", shown(repo.as_ref()), shown(number.as_ref()));
    return Err(
      Failure::new(
        "slot_mismatch",
        format!(
          "state file belongs to {owner}, asked for {}#{}",
          request.repo, request.number
        ),
      )
      .with("source", "state")
      .with("state", json!({"repo": repo, "number": number}))
      .with(
        "requested",
        json!({"repo": request.repo, "number": request.number}),
      ),
    );
  }
  Ok(Some(value))
}

fn shown(value: Option<&Value>) -> String {
  value.map_or_else(|| "null".to_owned(), crate::json::jq_string)
}

type Read = (Value, Option<String>, Option<GraphQlUsage>);

fn snapshot(request: &Request<'_>, source: Source<'_>, env: &Env) -> Result<Read> {
  match source {
    Source::File(path) => {
      let invalid = || {
        Failure::new(
          "invalid_json",
          format!("--snapshot-in is not a JSON file: {}", path.display()),
        )
        .with("source", "snapshot")
      };
      let raw = std::fs::read_to_string(path).ok().ok_or_else(invalid)?;
      let snapshot: Value = serde_json::from_str(&raw).ok().ok_or_else(invalid)?;
      let (repo, number) = (snapshot.get("repo"), snapshot.get("number"));
      if repo.and_then(Value::as_str) != Some(request.repo)
        || number.and_then(Value::as_u64) != Some(request.number)
      {
        let message = format!(
          "--snapshot-in is for {}#{}, asked for {}#{}",
          shown(repo),
          shown(number),
          request.repo,
          request.number
        );
        return Err(Failure::new("slot_mismatch", message).with("source", "snapshot"));
      }
      Ok((snapshot, Some(raw), None))
    }
    Source::GitHub {
      config,
      page_size,
      client,
    } => github(request, config, page_size, client, env),
  }
}

/// Collect `request` from GitHub, through the kept client when there is a slot.
fn github(
  request: &Request<'_>,
  config: Config,
  page_size: u64,
  kept: Option<&Mutex<Option<Client>>>,
  env: &Env,
) -> Result<Read> {
  let build = || Client::new(config, env).map_err(|err| collect::failure("client", &err));
  let request = collect::Request {
    repo: request.repo,
    number: request.number,
    page_size,
    now: request.now,
  };
  let collected = match kept {
    None => collect::collect(&build()?, &request)?,
    Some(slot) => {
      let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
      let client = match slot.take() {
        Some(client) => client,
        None => build()?,
      };
      let collected = collect::collect(&client, &request);
      *slot = Some(client);
      collected?
    }
  };
  Ok((collected.snapshot, None, Some(collected.graphql)))
}

pub(crate) fn valid(snapshot: &Value) -> Result<()> {
  let ok = snapshot.get("version").and_then(Value::as_u64) == Some(1)
    && snapshot.get("repo").is_some_and(Value::is_string)
    && snapshot.get("number").is_some_and(Value::is_number)
    && snapshot.pointer("/head/sha").is_some_and(Value::is_string)
    && snapshot.get("pr").is_some_and(Value::is_object)
    && snapshot.get("threads").is_some_and(Value::is_array);
  if ok {
    Ok(())
  } else {
    Err(
      Failure::new(
        "invalid_json",
        "snapshot is not a version-1 pr-babysit snapshot",
      )
      .with("source", "snapshot"),
    )
  }
}

/// Record `failure` as the previous state's `pr.lastError`; best effort.
fn stamp(request: &Request<'_>, previous: Option<Value>, failure: &Failure) {
  let Some(mut state) = previous else {
    return;
  };
  let error = json!({"code": failure.code, "message": failure.message, "at": request.now});
  if let Some(pr) = state.get_mut("pr").and_then(Value::as_object_mut) {
    pr.insert("lastError".into(), error);
    let _stamped = write_json(request.state_file, &state, "state");
  }
}

#[cfg(test)]
#[path = "tests/tick_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/tick_state_test.rs"]
mod state_tests;
