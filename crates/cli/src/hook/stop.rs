//! Stop once for each unread detached job in this worktree.

use toolu_engine::resources::{binding::resource_binding, job_results, store::resource_home};
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::git;
use toolu_runtime::host::detect::detect;

use super::HookCall;

/// Remind this worktree about one unread detached job before the host ends a turn.
pub(super) fn run(call: &HookCall<'_>) -> Outcome {
  let cwd = match (call.cwd)() {
    Ok(cwd) => cwd,
    Err(err) => {
      return block(
        call,
        &format!("toolu Stop: cannot read working directory: {err}"),
      );
    }
  };
  let Some(worktree) = git::toplevel(call.env, &cwd) else {
    return Outcome::notice(None);
  };
  let root = match resource_binding(&cwd) {
    Ok(Some(binding)) => binding.root,
    Ok(None) => resource_home(call.env),
    Err(err) => return block(call, &format!("toolu Stop: {err}")),
  };
  match job_results::block_stop_once(&root, &worktree) {
    Ok(Some(record)) => {
      let reason = format!(
        "detached job {} is pending or has an unread result; check it before stopping",
        record.id
      );
      block(call, &reason)
    }
    Ok(None) => Outcome::notice(None),
    Err(err) => block(call, &format!("toolu Stop: {err}")),
  }
}

fn block(call: &HookCall<'_>, reason: &str) -> Outcome {
  if detect(call.env, None).host == Host::Cursor {
    Outcome::data(serde_json::json!({"followup_message": reason}).to_string())
  } else {
    Outcome::failed(Exit::Blocked, format!("blocked: {reason}"))
  }
}

#[cfg(test)]
#[path = "tests/stop_test.rs"]
mod tests;
