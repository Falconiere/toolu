//! Stop a recorded hc worker only after verifying its identity and workload.

use super::super::failure::Failure;
use super::super::lifecycle::{Herdr, Outcome, exit_grace};
use super::super::record::{Record, text};
use crate::workload::{await_owned_exit, owned_workload};
use serde_json::Value;
use std::path::Path;
use toolu_mux::hc::{Client, require_owner};

pub(super) fn stop(record: &Record, herdr: &Herdr<'_>) -> Result<Outcome, Failure> {
  if crate::launch::hc::remote_worker(record) {
    return Err(Failure::refusal(
      "remote hc replacement requires remote workload verification",
    ));
  }
  let session = text(record, "hc_session").ok_or_else(|| Failure::refusal("missing hc session"))?;
  if session != herdr.env.get("HC_SESSION").unwrap_or("default") {
    return Err(Failure::refusal(
      "hc session differs from the recorded worker",
    ));
  }
  let address =
    text(record, "terminal_address").ok_or_else(|| Failure::refusal("missing hc address"))?;
  let owner = text(record, "terminal_owner").ok_or_else(|| Failure::refusal("missing hc owner"))?;
  if owner.strip_prefix("toolu.owner=") != text(record, "lease_token") {
    return Err(Failure::refusal("hc owner differs from the lease"));
  }
  let cwd = text(record, "worktree").ok_or_else(|| Failure::refusal("missing worktree"))?;
  let client = Client::new(herdr.env);
  let terminal = client
    .get(address)
    .map_err(|e| Failure::unknown(format!("hc {}: {}", e.code, e.message)))?;
  require_owner(&terminal, owner).map_err(|e| Failure::refusal(e.message))?;
  if terminal.get("cwd").and_then(Value::as_str) != Some(cwd) {
    return Err(Failure::refusal("hc worktree identity differs"));
  }
  let work = owned_workload(Path::new(cwd), None).map_err(Failure::unknown)?;
  if terminal.get("state").and_then(Value::as_str) != Some("finished") {
    client
      .kill_owned(address, owner)
      .map_err(|e| Failure::unknown(format!("hc {}: {}", e.code, e.message)))?;
  }
  if !await_owned_exit(Path::new(cwd), None, &work, exit_grace(herdr.env))
    .map_err(Failure::unknown)?
  {
    return Ok(Outcome::CleanupIncomplete);
  }
  Ok(Outcome::Exited)
}

#[cfg(test)]
#[path = "tests/hc_test.rs"]
mod tests;
