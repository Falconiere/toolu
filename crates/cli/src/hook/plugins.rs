//! The other plugins' native hooks, dispatched by [`super::run`] before the
//! lifecycle path. Each reports through the prelude's advisory, so they return
//! an [`Outcome`] directly: `agent-tier` and `mcp-tools` write their own
//! output, while the rest go through [`super::prefix`].

use std::path::Path;

use toolu_engine::quality::check_toolu;
use toolu_hub::tool_hook::{Phase, QUALITY_PLUGINS, plugin_tool_hook};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::host::detect::detect;

use crate::Context;
use crate::fast::HookRequest;

/// The other plugins' hooks: `agent-tier` and `mcp-tools`, jev, toolu-review,
/// ast-grep, git-better, the quality plugins, the statusline, pr-babysit and
/// epic-orchestrator.
/// Each reports through the prelude's advisory.
pub(super) fn dispatch(
  request: &HookRequest,
  context: &Context<'_>,
  advisory: Option<&String>,
) -> Option<Outcome> {
  if let Some(outcome) = routed_hook(request, context.stdin) {
    return Some(outcome);
  }
  if let Some(outcome) = jev_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = review_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = docs_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = ast_grep_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = git_better_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = quality_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = statusline_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = babysit_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  if let Some(outcome) = epic_hook(request, context) {
    return Some(super::prefix(advisory.cloned(), outcome));
  }
  None
}

/// The three epic-orchestrator `SessionStart` entries.
pub(super) fn epic_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "epic-orchestrator" {
    return None;
  }
  let env = (context.env)();
  match request.name.as_str() {
    "check-binary" => {
      let stdin = (context.stdin)().ok();
      Some(toolu_epic_orchestrator::hook::check_binary(
        &env,
        stdin.as_deref(),
      ))
    }
    "check-deps" => Some(toolu_epic_orchestrator::hook::check_deps(&env)),
    "engine-ensure" => Some(toolu_epic_orchestrator::hook::engine_ensure(&env)),
    _ => None,
  }
}

/// toolu's `agent-tier` and `mcp-tools`. They write their own output.
pub(super) fn routed_hook(
  request: &HookRequest,
  stdin: &dyn Fn() -> std::io::Result<String>,
) -> Option<Outcome> {
  if request.plugin != "toolu" {
    return None;
  }
  let root = request.plugin_root.as_deref().map(Path::new);
  match request.name.as_str() {
    "agent-tier" => Some(toolu_hub::agent_tier::hook(stdin())),
    "mcp-tools" => Some(toolu_hub::mcp_hook::hook(stdin(), root)),
    _ => None,
  }
}

/// ast-grep's manifest-publishing `SessionStart` hook.
pub(super) fn ast_grep_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "ast-grep" {
    return None;
  }
  let phase = match request.name.as_str() {
    "pre-tools" => Some(Phase::Pre),
    "post-tools" => Some(Phase::Post),
    _ => None,
  };
  if let Some(phase) = phase {
    return Some(plugin_tool_hook(
      "ast-grep@toolu",
      phase,
      (context.stdin)(),
      request.plugin_root.as_deref().map(Path::new),
    ));
  }
  if request.name != "register" {
    return None;
  }
  let env = (context.env)();
  Some(toolu_hub::ast_grep::register(&env, detect(&env, None).host))
}

/// git-better's `SessionStart` registration, its dispatcher dependency check and
/// the branch summary.
pub(super) fn git_better_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != toolu_hub::git_better::PLUGIN
    || request.event.as_deref() != Some("SessionStart")
  {
    return None;
  }
  let env = (context.env)();
  match request.name.as_str() {
    "register" => Some(toolu_hub::git_better::register(
      &env,
      detect(&env, None).host,
    )),
    "check-toolu" => Some(check_toolu(&env)),
    "session-start" => Some(toolu_hub::git_better::session_start(&env)),
    _ => None,
  }
}

/// A quality plugin's two `SessionStart` entries (the hub's `QUALITY_PLUGINS`) and
/// `post-tools`, which runs only that plugin's rules.
pub(super) fn quality_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  let plugin = QUALITY_PLUGINS
    .iter()
    .find(|plugin| plugin.plugin == request.plugin)?;
  let env = (context.env)();
  match request.name.as_str() {
    "register" => Some(plugin.register(&env, detect(&env, None).host)),
    "check-toolu" => Some(check_toolu(&env)),
    "post-tools" => Some(plugin_tool_hook(
      plugin.spec,
      Phase::Post,
      (context.stdin)(),
      plugin_root(request),
    )),
    _ => None,
  }
}

/// The statusline's native `SessionStart` entry.
pub(super) fn statusline_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "statusline"
    || request.name != "session-start"
    || request.event.as_deref() != Some("SessionStart")
  {
    return None;
  }
  let env = (context.env)();
  Some(toolu_statusline::session_start(
    &env,
    request.plugin_root.as_deref().map(Path::new),
  ))
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
      Some(root) => toolu_jev::session_start(&env, root),
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

/// Documentation review registration and its dispatcher dependency check.
pub(super) fn docs_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != toolu_documentation_gate::PLUGIN
    || request.event.as_deref() != Some("SessionStart")
  {
    return None;
  }
  let env = (context.env)();
  match request.name.as_str() {
    "register" => Some(toolu_engine::docs_review::register(
      &env,
      detect(&env, None).host,
    )),
    "check-toolu" => Some(check_toolu(&env)),
    _ => None,
  }
}

/// The two toolu-review `SessionStart` entries.
pub(super) fn review_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != "toolu-review" {
    return None;
  }
  let env = (context.env)();
  match request.name.as_str() {
    "session-start" => Some(match plugin_root(request) {
      Some(root) => toolu_review::session_start(&env, root),
      None => quiet(),
    }),
    "check-binary" => {
      let stdin = (context.stdin)().ok();
      Some(toolu_review::check_binary(&env, stdin.as_deref()))
    }
    _ => None,
  }
}

/// pr-babysit's two `SessionStart` entries.
pub(super) fn babysit_hook(request: &HookRequest, context: &Context<'_>) -> Option<Outcome> {
  if request.plugin != toolu_pr_babysit::PLUGIN || request.event.as_deref() != Some("SessionStart")
  {
    return None;
  }
  let env = (context.env)();
  match request.name.as_str() {
    "check-toolu" => Some(toolu_pr_babysit::check_toolu(&env)),
    "check-binary" => {
      let stdin = (context.stdin)().ok();
      Some(toolu_pr_babysit::check_binary(&env, stdin.as_deref()))
    }
    _ => None,
  }
}

/// The plugin directory the launcher passed. Empty is absent: it would
/// resolve the shim against the working directory.
pub(super) fn plugin_root(request: &HookRequest) -> Option<&Path> {
  request
    .plugin_root
    .as_deref()
    .filter(|root| !root.is_empty())
    .map(Path::new)
}

/// A successful hook without output.
pub(super) fn quiet() -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/plugins_test.rs"]
mod tests;
