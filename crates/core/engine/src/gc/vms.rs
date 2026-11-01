//! Idle VMs and gate images: the Lima sandbox (through `toolu sandbox`), the
//! Colima `toolu-gate` profile the deploy gate runs in, and Docker's dangling
//! images there. Nothing stops while a lease is held, a container runs, or
//! the program is not on `PATH`; each case is reported as it is.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use super::worktrees::on_path;
use super::{Request, capture};
use crate::resources::store::{leases, read_resource_state};

/// The Colima profile the `/deploy` skill and worker gates start.
pub(crate) const GATE_PROFILE: &str = "toolu-gate";
/// Colima's Docker context for that profile.
const GATE_CONTEXT: &str = "colima-toolu-gate";
/// How long a VM stop or an image prune may take.
const SLOW: Duration = Duration::from_secs(300);
const QUICK: Duration = Duration::from_secs(30);

/// Whether no agent or job lease is held under `root`.
///
/// # Errors
/// The resource state cannot be read.
pub fn idle(root: &Path) -> Result<bool, String> {
  read_resource_state(root).map(|state| leases(&state).is_empty())
}

/// Why VMs stay, when they do.
fn held(idle: &Result<bool, String>) -> Option<String> {
  match idle {
    Ok(true) => None,
    Ok(false) => Some("a worker lease is held".to_owned()),
    Err(err) => Some(format!("leases unknown: {err}")),
  }
}

/// A `toolu --json sandbox <verb>` document, or the failure.
fn toolu_json(request: &Request, toolu: &Path, verb: &[&str]) -> Result<Value, String> {
  let toolu = toolu.to_string_lossy();
  let mut argv = vec![toolu.as_ref(), "--json", "sandbox"];
  argv.extend_from_slice(verb);
  let text = capture(&request.env, None, &argv, SLOW)?;
  serde_json::from_str(&text).map_err(|err| format!("unreadable sandbox reply: {err}"))
}

/// Prune the sandbox's orphan clones, then stop it when idle.
pub(crate) fn sandbox(request: &Request, idle: &Result<bool, String>) -> Value {
  let Some(toolu) = request.toolu.as_deref() else {
    return json!({"action": "skipped", "reason": "no toolu binary"});
  };
  let mut prune = vec!["prune"];
  if request.apply {
    prune.push("--apply");
  }
  let clones = toolu_json(request, toolu, &prune).unwrap_or_else(|err| json!({"error": err}));
  let (action, reason) = match held(idle).or_else(|| shell_open(request)) {
    Some(reason) => ("kept", reason),
    None if !request.apply => ("would-stop", "no lease is held".to_owned()),
    None => match toolu_json(request, toolu, &["stop"]) {
      Ok(doc) if doc.get("changed") == Some(&Value::Bool(true)) => ("stopped", String::new()),
      Ok(doc) => (
        "kept",
        doc
          .get("state")
          .and_then(Value::as_str)
          .unwrap_or("not running")
          .to_owned(),
      ),
      Err(err) => ("kept", err),
    },
  };
  json!({"action": action, "reason": reason, "clones": clones})
}

/// Why the sandbox stays when a `limactl shell` (a `toolu sandbox run` or
/// `shell`) is running on this machine without an engine lease.
fn shell_open(request: &Request) -> Option<String> {
  capture(
    &request.env,
    None,
    &["pgrep", "-f", "limactl shell "],
    QUICK,
  )
  .ok()
  .filter(|pids| !pids.trim().is_empty())
  .map(|_| "a sandbox shell or run is active".to_owned())
}

/// The Colima gate profile's status, one of `Running`, `Stopped`, ...; `None` when absent.
fn colima_status(request: &Request) -> Result<Option<String>, String> {
  let text = capture(&request.env, None, &["colima", "list", "--json"], QUICK)?;
  Ok(text.lines().find_map(|line| {
    let row: Value = serde_json::from_str(line).ok()?;
    (row.get("name").and_then(Value::as_str) == Some(GATE_PROFILE)).then(|| {
      row
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
    })
  }))
}

/// Whether a container runs in the gate context; `None` when Docker cannot say.
fn gate_containers(request: &Request) -> Option<bool> {
  capture(
    &request.env,
    None,
    &["docker", "--context", GATE_CONTEXT, "ps", "-q"],
    QUICK,
  )
  .ok()
  .map(|text| !text.trim().is_empty())
}

/// Stop the Colima gate profile when it runs idle.
pub(crate) fn colima(request: &Request, idle: &Result<bool, String>) -> Value {
  if !on_path(&request.env, "colima") {
    return json!({GATE_PROFILE: "no-colima"});
  }
  let status = match colima_status(request) {
    Ok(Some(status)) => status,
    Ok(None) => return json!({GATE_PROFILE: "absent"}),
    Err(err) => return json!({GATE_PROFILE: "unknown", "reason": err}),
  };
  if status != "Running" {
    return json!({GATE_PROFILE: status.to_lowercase()});
  }
  if let Some(reason) = held(idle) {
    return json!({GATE_PROFILE: "kept", "reason": reason});
  }
  if gate_containers(request) != Some(false) {
    return json!({GATE_PROFILE: "kept", "reason": "a container runs in it, or docker did not answer"});
  }
  if !request.apply {
    return json!({GATE_PROFILE: "would-stop"});
  }
  match capture(&request.env, None, &["colima", "stop", GATE_PROFILE], SLOW) {
    Ok(_) => json!({GATE_PROFILE: "stopped"}),
    Err(err) => json!({GATE_PROFILE: "kept", "reason": err}),
  }
}

/// Dangling images in the gate context, pruned with `apply`; the gate image tags stay.
pub(crate) fn docker(request: &Request) -> Value {
  if !on_path(&request.env, "docker") {
    return json!({"skipped": "no docker"});
  }
  let context = ["docker", "--context", GATE_CONTEXT];
  let listed = capture(
    &request.env,
    None,
    &[&context[..], &["images", "--filter", "dangling=true", "-q"]].concat(),
    QUICK,
  );
  let dangling = match listed {
    Ok(text) => text.lines().filter(|line| !line.is_empty()).count(),
    Err(err) => return json!({"skipped": err}),
  };
  let tags = capture(
    &request.env,
    None,
    &[
      &context[..],
      &["images", "toolu-deploy-gate", "--format", "{{.Tag}}"],
    ]
    .concat(),
    QUICK,
  )
  .map(|text| text.lines().map(str::to_owned).collect::<Vec<_>>())
  .unwrap_or_default();
  if !request.apply || dangling == 0 {
    return json!({"dangling": dangling, "gate_image_tags": tags});
  }
  let pruned = capture(
    &request.env,
    None,
    &[&context[..], &["image", "prune", "--force"]].concat(),
    SLOW,
  );
  match pruned {
    Ok(text) => json!({"dangling": 0, "removed": dangling, "gate_image_tags": tags,
      "note": text.lines().last().unwrap_or("").trim()}),
    Err(err) => json!({"dangling": dangling, "gate_image_tags": tags, "error": err}),
  }
}

#[cfg(test)]
#[path = "tests/vms_test.rs"]
mod tests;
