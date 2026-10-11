//! `toolu [<plugin>] hook <name>`: the skew prelude of #411, then the named hook.
//!
//! The prelude owns the advisory and mismatch output. A mismatch never reaches
//! the hook: enforcing events exit 2 with `blocked:` on stderr, context events
//! print it as a `systemMessage`. A semver advisory is printed at `SessionStart`
//! only, on the line before the hook's own message: skew changes only on an
//! upgrade, so one line per session is the design (#411, the #412 scenario), and
//! repeating it on every prompt or tool call would be noise. Enforcement is the
//! same either way.

use std::path::{Path, PathBuf};

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
use toolu_runtime::startup::native_toolu_advice;

use crate::fast::HookRequest;
use crate::{Context, VERSION};

mod plugins;

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
  if let Some(outcome) = plugins::dispatch(request, context, advisory.as_ref()) {
    return outcome;
  }
  let env = (context.env)();
  let call = HookCall {
    request,
    stdin: context.stdin,
    exe: exe.as_deref(),
    advisory: advisory.as_deref(),
    env: &env,
    cwd: context.cwd,
  };
  if let Some(outcome) = toolu_hook(&call) {
    return outcome;
  }
  let result = dispatch(request, enforcing, upgrade);
  compose(advisory, result)
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
  cwd: &'a dyn Fn() -> std::io::Result<PathBuf>,
}

/// toolu's session lifecycle hooks (#424). The hub renders session-start, including
/// a skew advisory, so this path does not compose the stdout again.
fn toolu_hook(call: &HookCall<'_>) -> Option<Outcome> {
  if call.request.plugin != "toolu" {
    return None;
  }
  let name = call.request.name.as_str();
  if name == "check-binary" {
    return Some(check_binary(call));
  }
  if !matches!(name, "session-start" | "user-prompt-submit" | "pre-compact") {
    return None;
  }
  let cwd = match (call.cwd)() {
    Ok(cwd) => cwd,
    Err(err) => return Some(cwd_failure(name, call.advisory, &err)),
  };
  Some(match name {
    "session-start" => run_session(call, &cwd),
    "user-prompt-submit" => run_prompt(call, &cwd),
    _ => run_compact(call, &cwd),
  })
}

/// The former second `SessionStart` entry, now using the shared native notice.
fn check_binary(call: &HookCall<'_>) -> Outcome {
  let session_id = (call.stdin)()
    .ok()
    .and_then(|payload| serde_json::from_str::<serde_json::Value>(&payload).ok())
    .and_then(|payload| payload.get("session_id")?.as_str().map(str::to_owned));
  let session_id = session_id
    .as_deref()
    .filter(|id| !id.is_empty())
    .or_else(|| call.env.get("TOOLU_SESSION_ID"));
  Outcome {
    exit: Exit::Success,
    stdout: native_toolu_advice(call.env, session_id).map(|line| system_message(&line)),
    stderr: None,
  }
}

/// An unreadable working directory: the hook does not run, so it cannot guess
/// a root. Session start reports it the way an unreadable payload does; the
/// other two follow their stdin errors.
fn cwd_failure(name: &str, advisory: Option<&str>, err: &std::io::Error) -> Outcome {
  let detail = format!("the working directory could not be read: {err}");
  if name == "session-start" {
    return reported(
      advisory,
      &format!("toolu runtime: native {VERSION}, but {detail}"),
    );
  }
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: Some(format!("toolu {name}: {detail}")),
  }
}

fn run_session(call: &HookCall<'_>, cwd: &Path) -> Outcome {
  let plugin_root = call.request.plugin_root.as_deref().map(Path::new);
  match (call.stdin)() {
    Ok(payload) => session_start(&SessionInput {
      env: call.env,
      cwd,
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
  reported(advisory, &line)
}

fn reported(advisory: Option<&str>, line: &str) -> Outcome {
  let text = match advisory.filter(|line| !line.is_empty()) {
    Some(advisory) => format!("{advisory}\n{line}"),
    None => line.to_owned(),
  };
  Outcome {
    exit: Exit::Success,
    stdout: Some(system_message(&text)),
    stderr: None,
  }
}

fn run_prompt(call: &HookCall<'_>, cwd: &Path) -> Outcome {
  match (call.stdin)() {
    Ok(payload) => prompt_submit(&PromptInput {
      env: call.env,
      cwd,
      payload: &payload,
    }),
    Err(err) => hook_error("user-prompt-submit", &err),
  }
}

fn run_compact(call: &HookCall<'_>, cwd: &Path) -> Outcome {
  match (call.stdin)() {
    Ok(_) => pre_compact(&PreCompactInput { env: call.env, cwd }),
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
