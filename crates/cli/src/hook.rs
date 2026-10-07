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

use toolu_hub::lifecycle::pre_compact::{PreCompactInput, pre_compact};
use toolu_hub::lifecycle::prompt_submit::{PromptInput, prompt_submit};
use toolu_hub::lifecycle::session::{SessionInput, session_start};
use toolu_hub::tool_hook::{Phase, tool_hook};
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::event::is_enforcing;
use toolu_protocol::exit::Exit;
use toolu_protocol::install::INSTALLER;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest;
use toolu_runtime::skew::{Binary, Caller, Skew, assess};

use crate::fast::HookRequest;
use crate::{Context, VERSION};

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
  let cwd = (context.cwd)();
  let env = (context.env)();
  let call = HookCall {
    request,
    stdin: context.stdin,
    exe: exe.as_deref(),
    advisory: advisory.as_deref(),
    env: &env,
    cwd: &cwd,
  };
  if let Some(outcome) = toolu_hook(&call) {
    return outcome;
  }
  let result = dispatch(request, enforcing, upgrade);
  compose(advisory, result)
}

/// The three jev hooks. Anything else stays the "has no hook" path.
fn jev_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "jev" {
    return None;
  }
  let env = (context.env)();
  let stdin = (context.stdin)().ok();
  let text = stdin.as_deref();
  let outcome = match request.name.as_str() {
    "session-start" => match plugin_root(request) {
      Some(root) => toolu_jev::session_start(&env, root, text),
      None => quiet(),
    },
    "user-prompt-submit" => match plugin_root(request) {
      Some(root) => toolu_jev::user_prompt_submit(&env, root, text),
      None => quiet(),
    },
    "check-binary" => toolu_jev::check_binary(&env, text),
    _ => return None,
  };
  Some(outcome)
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

struct HookCall<'a> {
  request: &'a HookRequest,
  stdin: &'a dyn Fn() -> std::io::Result<String>,
  exe: Option<&'a Path>,
  advisory: Option<&'a str>,
  env: &'a Env,
  cwd: &'a Path,
}

/// toolu's session lifecycle hooks (#424). The hub renders session-start, including
/// a skew advisory, so this path does not compose the stdout again.
fn toolu_hook(call: &HookCall<'_>) -> Option<Outcome> {
  if call.request.plugin != "toolu" {
    return None;
  }
  match call.request.name.as_str() {
    "session-start" => Some(run_session(call)),
    "user-prompt-submit" => Some(run_prompt(call)),
    "pre-compact" => Some(run_compact(call)),
    _ => None,
  }
}

fn run_session(call: &HookCall<'_>) -> Outcome {
  let plugin_root = call.request.plugin_root.as_deref().map(Path::new);
  match (call.stdin)() {
    Ok(payload) => session_start(&SessionInput {
      env: call.env,
      cwd: call.cwd,
      plugin_root,
      version: VERSION,
      exe: call.exe,
      advisory: call.advisory,
      payload: &payload,
    }),
    Err(err) => unread_payload(call.advisory, &err),
  }
}

fn unread_payload(advisory: Option<&str>, err: &std::io::Error) -> Outcome {
  let line =
    format!("toolu runtime: native {VERSION}, but the hook payload could not be read: {err}");
  let text = match advisory.filter(|line| !line.is_empty()) {
    Some(advisory) => format!("{advisory}\n{line}"),
    None => line,
  };
  Outcome {
    exit: Exit::Success,
    stdout: Some(system_message(&text)),
    stderr: None,
  }
}

fn run_prompt(call: &HookCall<'_>) -> Outcome {
  match (call.stdin)() {
    Ok(payload) => prompt_submit(&PromptInput {
      env: call.env,
      cwd: call.cwd,
      payload: &payload,
    }),
    Err(err) => hook_error("user-prompt-submit", &err),
  }
}

fn run_compact(call: &HookCall<'_>) -> Outcome {
  match (call.stdin)() {
    Ok(_) => pre_compact(&PreCompactInput {
      env: call.env,
      cwd: call.cwd,
    }),
    Err(err) => hook_error("pre-compact", &err),
  }
}

fn hook_error(name: &str, err: &std::io::Error) -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: Some(format!("toolu {name}: {err}")),
  }
}

/// A hook this binary does not run. Another plugin's `session-start` included,
/// it reports "has no hook": a `systemMessage` on a context event, a block on
/// an enforcing one.
fn dispatch(request: &HookRequest, enforcing: bool, upgrade: &str) -> HookResult {
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
