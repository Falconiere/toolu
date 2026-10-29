//! Finish a terminal owned by one herdr-app launch attempt.

use std::path::Path;
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_mux::hc::{Client, require_owner};

use super::ownership::{fence_cleanup_ownership, verify_no_jobs};
use super::prepare::{await_quiet, capture_work};
use super::record::{RemovalIntent, removal_intent};
use super::remove::{Removal, Teardown, remove_gone_worktree, worktree_registered};
use crate::launch::Ctx;
use crate::launch::record::{Record, text, write_record};
use crate::workload::{owned_workload, verify_owned_exit};

pub(super) fn stop(ctx: &Ctx<'_>, record: &Record) -> Result<String, String> {
  let address = text(record, "terminal_address")
    .filter(|value| !value.is_empty())
    .ok_or("missing hc terminal address")?;
  let owner = text(record, "terminal_owner")
    .filter(|value| value.starts_with("toolu.owner=") && value.len() > "toolu.owner=".len())
    .ok_or("missing hc terminal ownership")?;
  let client = Client::new(ctx.env);
  let terminal = match client.get(address) {
    Ok(terminal) => terminal,
    Err(err) if err.code == "not_found" => return Ok(address.to_owned()),
    Err(err) => return Err(format!("hc {}: {}", err.code, err.message)),
  };
  require_owner(&terminal, owner).map_err(|err| format!("hc {}: {}", err.code, err.message))?;
  if terminal.get("state").and_then(Value::as_str) != Some("finished") {
    match client.kill_owned(address, owner) {
      Ok(()) => {}
      Err(err) if err.code == "not_found" => {}
      Err(err) => return Err(format!("hc {}: {}", err.code, err.message)),
    }
  }
  Ok(address.to_owned())
}

pub(super) fn remote(record: &Record) -> bool {
  ["machine", "remote_repo", "remote_worktree"]
    .iter()
    .any(|field| record.contains_key(*field))
}

pub(super) fn tear_down(
  ctx: &mut Ctx<'_>,
  teardown: &mut Teardown<'_>,
  state: &str,
  key: &str,
  record: &mut Record,
) -> Result<Removal, String> {
  if remote(record) {
    super::hc_remote::remote_teardown(ctx, teardown, state, key, record)?;
  }
  let ownership = fence_cleanup_ownership(teardown.path, state, key, record)?;
  let worktree = text(record, "worktree")
    .ok_or("missing worktree")?
    .to_owned();
  let absent = !Path::new(&worktree).exists();
  let intent = removal_intent(record)?;
  let owned = if absent {
    Vec::new()
  } else {
    owned_workload(Path::new(&worktree), None)?
  };
  let address = stop(ctx, record)?;
  if absent {
    return finish_absent(
      ctx,
      teardown,
      record,
      ownership.as_ref(),
      (&worktree, &address, intent),
    );
  }
  finish_live(
    ctx,
    teardown,
    record,
    ownership.as_ref(),
    (worktree, address, owned),
  )
}

fn finish_absent(
  ctx: &Ctx<'_>,
  teardown: &Teardown<'_>,
  record: &mut Record,
  ownership: Option<&super::ownership::Cleanup>,
  identity: (&str, &str, Option<RemovalIntent>),
) -> Result<Removal, String> {
  let (worktree, address, intent) = identity;
  let intent = intent.ok_or("cannot verify hc cleanup without removal intent")?;
  if intent.workspace_id != address || intent.worktree != worktree {
    return Err("removal intent does not match recorded terminal ownership".to_owned());
  }
  if !verify_owned_exit(&intent.workload)? {
    return Err("owned background workload remains alive after worktree removal".to_owned());
  }
  verify_no_jobs(ownership, worktree)?;
  if worktree_registered(record, &intent, ctx.env)? {
    return Err("worktree path is absent but remains registered".to_owned());
  }
  teardown.removed(ctx, record, intent.wip_ref, intent.leftover_files)
}

fn finish_live(
  ctx: &Ctx<'_>,
  teardown: &Teardown<'_>,
  record: &mut Record,
  ownership: Option<&super::ownership::Cleanup>,
  input: (String, String, Vec<crate::workload::OwnedWorkload>),
) -> Result<Removal, String> {
  let (worktree, address, owned) = input;
  await_quiet(ctx, &worktree, None, &owned, false)?;
  verify_no_jobs(ownership, &worktree)?;
  let (wip_ref, dirty) = capture_work(ctx, record, &worktree, false, teardown.abandon)?;
  if !owned_workload(Path::new(&worktree), None)?.is_empty() {
    return Err("still exiting: new workload appeared before cleanup".to_owned());
  }
  let intent = RemovalIntent {
    workspace_id: address,
    worktree,
    wip_ref,
    leftover_files: dirty,
    workload: owned,
  };
  record.insert("stage".to_owned(), json!("cleanup-incomplete"));
  record.insert(
    "removal_intent".to_owned(),
    intent.to_value(&toolu_state::time::iso_millis(SystemTime::now())),
  );
  write_record(teardown.path, record)?;
  remove_gone_worktree(record, &intent, teardown.abandon, ctx.env)?;
  teardown.removed(ctx, record, intent.wip_ref, intent.leftover_files)
}

#[cfg(test)]
#[path = "tests/hc_test.rs"]
mod tests;
