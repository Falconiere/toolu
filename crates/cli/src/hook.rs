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
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest;
use toolu_runtime::skew::{Binary, Caller, Skew, assess};

use toolu_hub::tool_hook::{Phase, tool_hook};

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
  if let Some(phase) = tool_phase(request) {
    let plugin_root = request.plugin_root.as_deref().map(Path::new);
    return tool_hook(phase, (context.stdin)(), plugin_root);
  }
  if request.plugin == "toolu" && request.name == "agent-tier" {
    return toolu_hub::agent_tier::hook((context.stdin)());
  }
  if request.plugin == "toolu" && request.name == "mcp-tools" {
    return toolu_hub::mcp_hook::hook(
      (context.stdin)(),
      request.plugin_root.as_deref().map(Path::new),
    );
  }
  if let Some(outcome) = jev_hook(request, context) {
    return prefix(advisory, outcome);
  }
  if let Some(outcome) = review_hook(request, context) {
    return prefix(advisory, outcome);
  }
  if let Some(outcome) = ast_grep_hook(request, context) {
    return prefix(advisory, outcome);
  }
  let result = dispatch(request, context, exe.as_deref(), enforcing, upgrade);
  compose(advisory, result)
}

/// ast-grep's manifest-publishing `SessionStart` hook.
fn ast_grep_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "ast-grep" {
    return None;
  }
  let phase = match request.name.as_str() {
    "pre-tools" => Some(Phase::Pre),
    "post-tools" => Some(Phase::Post),
    _ => None,
  };
  if let Some(phase) = phase {
    return Some(toolu_hub::tool_hook::ast_grep_tool_hook(
      phase,
      (context.stdin)(),
      request.plugin_root.as_deref().map(Path::new),
    ));
  }
  if request.name != "register" {
    return None;
  }
  let owned;
  let env = if let Some(env) = context.env {
    env
  } else {
    owned = Env::process();
    &owned
  };
  Some(toolu_hub::ast_grep::register(env, detect(env, None).host))
}

/// The three jev hooks. Anything else stays the "has no hook" path.
fn jev_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "jev" {
    return None;
  }
  let owned;
  let env = if let Some(env) = context.env {
    env
  } else {
    owned = Env::process();
    &owned
  };
  let stdin = (context.stdin)().ok();
  let text = stdin.as_deref();
  let outcome = match request.name.as_str() {
    "session-start" => match plugin_root(request) {
      Some(root) => toolu_jev::session_start(env, root, text),
      None => quiet(),
    },
    "user-prompt-submit" => match plugin_root(request) {
      Some(root) => toolu_jev::user_prompt_submit(env, root, text),
      None => quiet(),
    },
    "check-binary" => toolu_jev::check_binary(env, text),
    _ => return None,
  };
  Some(outcome)
}

/// The two toolu-review `SessionStart` entries.
fn review_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "toolu-review" {
    return None;
  }
  let owned;
  let env = if let Some(env) = context.env {
    env
  } else {
    owned = Env::process();
    &owned
  };
  match request.name.as_str() {
    "session-start" => Some(match plugin_root(request) {
      Some(root) => toolu_review::session_start(env, root),
      None => quiet(),
    }),
    "check-binary" => {
      let stdin = (context.stdin)().ok();
      Some(toolu_review::check_binary(env, stdin.as_deref()))
    }
    _ => None,
  }
}

/// The plugin directory the launcher passed. Empty is absent: it would
/// resolve the shim against the working directory.
fn plugin_root(request: &HookRequest) -> Option<&Path> {
  request
    .plugin_root
    .as_deref()
    .filter(|root| !root.is_empty())
    .map(Path::new)
}

fn quiet() -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: None,
  }
}

/// A `SessionStart` skew advisory is its own `systemMessage` line, before the hook JSON.
fn prefix(advisory: Option<String>, outcome: Outcome) -> Outcome {
  let Some(text) = advisory else {
    return outcome;
  };
  let line = system_message(&text);
  Outcome {
    stdout: Some(match outcome.stdout {
      Some(body) => format!("{line}\n{body}"),
      None => line,
    }),
    ..outcome
  }
}

/// The tool hook `request` names: toolu's `pre-tools` or `post-tools` (#418),
/// the engine's dispatch, which writes its own output.
fn tool_phase(request: &HookRequest) -> Option<Phase> {
  match (request.plugin.as_str(), request.name.as_str()) {
    ("toolu", "pre-tools") => Some(Phase::Pre),
    ("toolu", "post-tools") => Some(Phase::Post),
    _ => None,
  }
}

/// The named hook. toolu's `session-start` carries the runtime diagnostic.
/// jev's three hooks are dispatched above. Every other name reports "has no hook":
/// a `systemMessage` on a context event such as `SessionStart`, a block on an enforcing one.
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

#[cfg(test)]
#[path = "tests/hook_jev_test.rs"]
mod jev_tests;
