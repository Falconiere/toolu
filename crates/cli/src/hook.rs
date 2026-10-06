//! `toolu [<plugin>] hook <name>`: the skew prelude of #411, then the named hook.
//!
//! The prelude owns the advisory and mismatch output. A mismatch never reaches
//! the hook: enforcing events exit 2 with `blocked:` on stderr, context events
//! print it as a `systemMessage`. A semver advisory is printed at `SessionStart`
//! only, on the line before the hook's own message: skew changes only on an
//! upgrade, so one line per session is the design (#411, the #412 scenario), and
//! repeating it on every prompt or tool call would be noise. Enforcement is the
//! same either way.

use std::path::Path;

use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::event::is_enforcing;
use toolu_protocol::exit::Exit;
use toolu_protocol::install::INSTALLER;
use toolu_runtime::cli::Outcome;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest;
use toolu_runtime::skew::{Binary, Caller, Skew, assess};

use crate::fast::HookRequest;
use crate::{Context, VERSION, session_start};

/// What a native hook decided before the prelude's advisory is added.
struct HookResult {
  exit: Exit,
  message: Option<String>,
  stderr: Option<String>,
}

/// Run the prelude and the hook `request` names.
pub(crate) fn run(request: &HookRequest, context: &Context<'_>) -> Outcome {
  let enforcing = request.event.as_deref().is_none_or(is_enforcing);
  let exe = (context.exe)();
  let upgrade = exe.as_deref().map_or(INSTALLER, upgrade_command);
  let skew = match &request.plugin_root {
    Some(root) => {
      let caller = Caller {
        plugin: &request.plugin,
        root: Path::new(root),
      };
      let binary = Binary {
        version: VERSION,
        protocol: HOOK_PROTOCOL,
      };
      assess(&caller, &binary, &manifest::read(Path::new(root)), upgrade)
    }
    None => Skew::Same,
  };
  let advisory = match skew {
    Skew::Mismatch(text) => return refuse(&text, enforcing),
    Skew::Advise(text) if request.event.as_deref() == Some("SessionStart") => Some(text),
    Skew::Advise(_) | Skew::Same => None,
  };
  let result = dispatch(request, context, exe.as_deref(), enforcing, upgrade);
  compose(advisory, result)
}

/// The named hook. Only toolu's `session-start` is native so far, as the spec
/// of #412 sets: it carries the runtime diagnostic that toolu's Bun
/// `session-start` prints today. Every other hook, another plugin's
/// `session-start` included, is ported by its own issue (#424, #430-#432);
/// until then a native entry for it reports "has no hook" — a `systemMessage`
/// on a context event such as `SessionStart`, a block on an enforcing one.
fn dispatch(
  request: &HookRequest,
  context: &Context<'_>,
  exe: Option<&Path>,
  enforcing: bool,
  upgrade: &str,
) -> HookResult {
  if request.plugin == "toolu" && request.name == "session-start" {
    let message = match (context.stdin)() {
      Ok(payload) => session_start::diagnostic(&payload, VERSION, exe),
      Err(err) => Some(format!(
        "toolu runtime: native {VERSION}, but the hook payload could not be read: {err}"
      )),
    };
    return HookResult {
      exit: Exit::Success,
      message,
      stderr: None,
    };
  }
  let text = format!(
    "{} plugin: toolu {VERSION} has no hook {}; upgrade it: {upgrade}",
    request.plugin, request.name
  );
  if enforcing {
    HookResult {
      exit: Exit::Blocked,
      message: None,
      stderr: Some(format!("blocked: {text}")),
    }
  } else {
    HookResult {
      exit: Exit::Success,
      message: Some(text),
      stderr: None,
    }
  }
}

/// A mismatch: block an enforcing event, report it on a context event.
fn refuse(text: &str, enforcing: bool) -> Outcome {
  if enforcing {
    Outcome {
      exit: Exit::Blocked,
      stdout: None,
      stderr: Some(format!("blocked: {text}")),
    }
  } else {
    Outcome {
      exit: Exit::Success,
      stdout: Some(system_message(text)),
      stderr: None,
    }
  }
}

/// One stdout JSON joining the advisory and the hook's message; nothing when both are empty.
fn compose(advisory: Option<String>, result: HookResult) -> Outcome {
  let lines: Vec<String> = advisory.into_iter().chain(result.message).collect();
  Outcome {
    exit: result.exit,
    stdout: (!lines.is_empty()).then(|| system_message(&lines.join("\n"))),
    stderr: result.stderr,
  }
}

fn system_message(text: &str) -> String {
  serde_json::json!({ "systemMessage": text }).to_string()
}

#[cfg(test)]
#[path = "tests/hook_test.rs"]
mod tests;
