//! A hook never fails anonymously (#642). Codex drops a failed context hook's
//! stderr, so a context hook's panic, or an exit its host gives no meaning, becomes
//! an exit-0 `systemMessage` naming the hook and its event. Cursor gets the same
//! line on stderr with exit 1, so it still counts as a failed hook there. An
//! enforcing hook's panic blocks with its message. A signal cannot be caught here:
//! the launcher `exec`s context hooks so a host timeout reaches toolu itself.

use toolu_protocol::exit::Exit;
use toolu_protocol::hook::panic::catch;
use toolu_protocol::host::Host;
use toolu_protocol::launcher::{Target, hook_label};
use toolu_runtime::cli::Outcome;

use crate::fast::HookRequest;

/// Run `hook` for `request`; `host` is detected only when a failure is reported.
pub(super) fn guarded(
  request: &HookRequest,
  enforcing: bool,
  host: impl FnOnce() -> Host,
  hook: impl FnOnce() -> Outcome,
) -> Outcome {
  let event = request.event.as_deref().unwrap_or("no event");
  let detail = match catch(hook) {
    Ok(outcome) if enforcing || in_contract(event, outcome.exit) => return outcome,
    Ok(outcome) => format!(
      "failed with exit {}: {}",
      outcome.exit.code(),
      outcome.stderr.as_deref().map_or("no message", str::trim)
    ),
    Err(message) => format!("panicked: {message}"),
  };
  let target = Target {
    plugin: &request.plugin,
    event,
    name: &request.name,
  };
  let text = format!("{} {detail}. See docs/install.md.", hook_label(&target));
  if enforcing {
    Outcome::failed(Exit::Blocked, format!("blocked: {text}"))
  } else if host() == Host::Cursor {
    Outcome::failed(Exit::Failure, text)
  } else {
    Outcome::notice(Some(text))
  }
}

/// Exit 0, or 2 where the host blocks the prompt or feeds stderr back to the model.
/// `SessionStart` and `PreCompact` give 2 no meaning on Codex, so it is reported.
fn in_contract(event: &str, exit: Exit) -> bool {
  exit == Exit::Success
    || (exit == Exit::Blocked && matches!(event, "UserPromptSubmit" | "PostToolUse"))
}

#[cfg(test)]
#[path = "tests/guard_test.rs"]
mod tests;
