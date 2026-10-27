//! Control-socket host pause and resume: the state thread applies the
//! shared-store change and journals it, naming who acted.

use std::sync::mpsc::Sender;

use serde_json::Value;

use crate::dispatch::{pump_after, result_value};
use crate::journal::Record;
use crate::server::Engine;

/// Apply one `host-pause` or `host-resume` request and reply.
pub(crate) fn reply_host_pause(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
  pause: bool,
) -> Result<bool, String> {
  let by = request.get("by").and_then(Value::as_str).unwrap_or("user");
  let result = crate::hosts::host_kind(request.get("host").and_then(Value::as_str).unwrap_or(""))
    .and_then(|kind| {
      if pause {
        crate::host_pause::pause_host(&engine.paths, kind, by, engine.world.now_ms)
      } else {
        let extra: Vec<String> = engine
          .world
          .epics
          .values()
          .map(|epic| epic.state_dir.clone())
          .collect();
        let dirs = crate::host_pause::epic_state_dirs(&engine.paths, &extra);
        crate::host_pause::resume_host(&engine.paths, kind, &dirs)
      }
      .map(|()| kind)
    })
    .and_then(|kind| journal_host_pause(engine, pause, kind.name(), by));
  let _sent = reply.send(result_value(result));
  if pause { Ok(false) } else { pump_after(engine) }
}

/// `action host-paused|host-resumed`, keyed by the host, naming who acted.
fn journal_host_pause(
  engine: &mut Engine,
  pause: bool,
  host: &str,
  by: &str,
) -> Result<(), String> {
  let name = if pause { "host-paused" } else { "host-resumed" };
  let record = Record::new("action", name, host, "", &format!("by={by}"));
  engine
    .world
    .outbox
    .push_back(crate::model::Step::Journal(record));
  engine.flush()
}

#[cfg(test)]
#[path = "tests/dispatch_hosts_test.rs"]
mod tests;
