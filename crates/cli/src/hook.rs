//! `toolu [<plugin>] hook <name>`: the skew prelude of #411, then the named hook.
//!
//! The prelude owns the advisory and mismatch output. A mismatch never reaches
//! the hook: enforcing events exit 2 with `blocked:` on stderr, context events
//! print it as a `systemMessage`. A semver advisory is printed at `SessionStart`
//! only, on the line before the hook's own message.

use std::path::Path;

use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::event::is_enforcing;
use toolu_protocol::install::INSTALLER;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest;
use toolu_runtime::skew::{Binary, Caller, Skew, assess};

use crate::args::HookRequest;
use crate::{Context, Outcome, VERSION, session_start};

/// What a native hook decided before the prelude's advisory is added.
struct HookResult {
  code: u8,
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

/// The named hook. Only toolu's `session-start` is native so far (#424 ports the rest).
fn dispatch(
  request: &HookRequest,
  context: &Context<'_>,
  exe: Option<&Path>,
  enforcing: bool,
  upgrade: &str,
) -> HookResult {
  if request.plugin == "toolu" && request.name == "session-start" {
    let message = session_start::diagnostic(&(context.stdin)(), VERSION, exe);
    return HookResult {
      code: 0,
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
      code: 2,
      message: None,
      stderr: Some(format!("blocked: {text}")),
    }
  } else {
    HookResult {
      code: 0,
      message: Some(text),
      stderr: None,
    }
  }
}

/// A mismatch: block an enforcing event, report it on a context event.
fn refuse(text: &str, enforcing: bool) -> Outcome {
  if enforcing {
    Outcome {
      code: 2,
      stdout: None,
      stderr: Some(format!("blocked: {text}")),
    }
  } else {
    Outcome {
      code: 0,
      stdout: Some(system_message(text)),
      stderr: None,
    }
  }
}

/// One stdout JSON joining the advisory and the hook's message; nothing when both are empty.
fn compose(advisory: Option<String>, result: HookResult) -> Outcome {
  let lines: Vec<String> = advisory.into_iter().chain(result.message).collect();
  Outcome {
    code: result.code,
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
