//! `toolu hook session-start` (#424). Housekeeping runs first. A disabled hook
//! still prints the title and the native runtime line on startup and resume.

use std::path::{Path, PathBuf};

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::load::{LoadedConfig, load};
use toolu_runtime::config::permissions::{PermissionsResult, permissions_autowrite};
use toolu_runtime::config::read::enabled;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::startup::context::render_hook_output;
use toolu_state::ctx::StateCtx;
use toolu_state::sweeper::sweep_state;

use crate::lifecycle::dependencies::dependency_warning;
use crate::lifecycle::diagnostics::diagnostics;
use crate::lifecycle::docs::{DocInput, doc_parts, event_title};
use crate::lifecycle::housekeeping::housekeeping;
use crate::lifecycle::mandates::{mandate_block, missing_tools_warning};
use crate::lifecycle::notices::{delivery_flow_notice, gate_preset_notice};
use crate::lifecycle::project::{branch_line, git_toplevel, project_facts};
use crate::lifecycle::text::{session_event, strip_trailing_newlines};

/// What the native session-start hook reads. The caller owns the process.
pub struct SessionInput<'a> {
  /// The hook's environment.
  pub env: &'a Env,
  /// The directory the hook was started in.
  pub cwd: &'a Path,
  /// `--plugin-root` when the launcher passed one.
  pub plugin_root: Option<&'a Path>,
  /// This binary's version, for the runtime line.
  pub version: &'a str,
  /// This binary's path. Unknown when the lookup failed.
  pub exe: Option<&'a Path>,
  /// A `SessionStart` skew advisory, printed on the line before the hook's own message.
  pub advisory: Option<&'a str>,
  /// The hook payload. Empty and invalid JSON are the `startup` event.
  pub payload: &'a str,
}

struct ContextIn<'a> {
  event: &'a str,
  env: &'a Env,
  host: Host,
  cwd: &'a Path,
  config_root: &'a Path,
  config: &'a LoadedConfig,
  plugin_root: Option<&'a Path>,
}

/// Run session-start. Failures become an empty stdout and a `toolu session-start` line.
pub fn session_start(input: &SessionInput<'_>) -> Outcome {
  let detected = detect(input.env, None);
  let roots = Roots::new(input.env.clone(), Some(detected.host));
  let config_root = roots.config_root();
  housekeeping(&roots);
  let event = session_event(input.payload);
  let config = load(&roots, Some(input.cwd));
  let (stdout, swept) = render(input, &event, detected.host, &config_root, &config);
  Outcome {
    exit: Exit::Success,
    stdout,
    stderr: diagnostics(detected.warning, &config, swept),
  }
}

fn render(
  input: &SessionInput<'_>,
  event: &str,
  host: Host,
  config_root: &Path,
  config: &LoadedConfig,
) -> (Option<String>, Vec<String>) {
  if !enabled(config, "hooks", "session-start") {
    return (disabled_stdout(event, input), Vec::new());
  }
  let built = ContextIn {
    event,
    env: input.env,
    host,
    cwd: input.cwd,
    config_root,
    config,
    plugin_root: input.plugin_root,
  };
  let (parts, warnings) = context_parts(&built);
  let message = full_message(event, input);
  (
    Some(hook_json(Some(&parts.join("\n\n")), &message)),
    warnings,
  )
}

fn disabled_stdout(event: &str, input: &SessionInput<'_>) -> Option<String> {
  if event == "startup" || event == "resume" {
    return Some(hook_json(None, &full_message(event, input)));
  }
  input
    .advisory
    .filter(|line| !line.is_empty())
    .map(|line| hook_json(None, line))
}

fn full_message(event: &str, input: &SessionInput<'_>) -> String {
  with_advisory(
    input.advisory,
    runtime_body(event, input.version, input.exe),
  )
}

fn runtime_body(event: &str, version: &str, exe: Option<&Path>) -> String {
  let title = event_title(event);
  if event != "startup" && event != "resume" {
    return title.to_owned();
  }
  let runtime = runtime_line(version, exe);
  if title.is_empty() {
    runtime
  } else {
    format!("{title}\n{runtime}")
  }
}

fn runtime_line(version: &str, exe: Option<&Path>) -> String {
  let at = exe.map_or_else(
    || "an unknown path".to_owned(),
    |path| path.display().to_string(),
  );
  format!("toolu runtime: native {version} at {at}")
}

fn with_advisory(advisory: Option<&str>, body: String) -> String {
  match advisory.filter(|line| !line.is_empty()) {
    Some(line) if body.is_empty() => line.to_owned(),
    Some(line) => format!("{line}\n{body}"),
    None => body,
  }
}

fn context_parts(input: &ContextIn<'_>) -> (Vec<String>, Vec<String>) {
  let top = git_toplevel(input.env, input.cwd);
  let outside_git = top.is_empty();
  let verbose = verbose(input.env);
  let facts = project_facts(nonempty(&top), input.env, verbose);
  let project = if outside_git {
    input.cwd.display().to_string()
  } else {
    top
  };
  let docs = docs_dir(input.env, input.host, input.plugin_root);
  let mut parts = doc_parts(&DocInput {
    docs: &docs,
    event: input.event,
    config: input.config,
    host: input.host,
    facts: &facts,
    verbose,
  });
  if !facts.name.is_empty() {
    parts.push(format!("Project: {}", facts.name));
  }
  let warnings = sweep_warnings(input.env, input.host, Path::new(&project), outside_git);
  for part in extras(input, &project) {
    push_present(&mut parts, part);
  }
  (parts, warnings)
}

fn extras(input: &ContextIn<'_>, project: &str) -> Vec<Option<String>> {
  vec![
    permissions_notice(input.config, input.env, Path::new(project), input.cwd),
    gate_preset_notice(input.config_root, &input.config.data),
    delivery_flow_notice(input.config_root, input.host),
    missing_tools_warning(input.env, input.config),
    mandate_block(input.config, input.env, input.host),
    dependency_warning(
      input.env,
      input.host,
      &plugin_root_of(input.env, input.host),
      project,
    ),
    branch_line(input.env, input.cwd),
  ]
}

fn permissions_notice(
  config: &LoadedConfig,
  env: &Env,
  project: &Path,
  cwd: &Path,
) -> Option<String> {
  let held = config.take_warnings();
  let result = permissions_autowrite(config, env, Some(project), Some(cwd));
  let _quiet = config.take_warnings();
  for message in held {
    config.warn(message);
  }
  match result {
    PermissionsResult::Written { notice, .. } => notice.filter(|text| !text.is_empty()),
    PermissionsResult::Skipped(_) => None,
  }
}

fn sweep_warnings(env: &Env, host: Host, project: &Path, outside_git: bool) -> Vec<String> {
  let mut ctx = StateCtx::new(Roots::new(env.clone(), Some(host)));
  sweep_state(&mut ctx, Some(project));
  ctx
    .warnings
    .into_iter()
    .filter(|warning| warning.starts_with("toolu-sweep:"))
    .filter(|warning| !outside_git || !warning.contains("cannot list the branches"))
    .collect()
}

fn docs_dir(env: &Env, host: Host, plugin_root: Option<&Path>) -> PathBuf {
  match plugin_root {
    Some(root) => root.join("hooks").join("docs"),
    None => Roots::new(env.clone(), Some(host))
      .plugin_root()
      .map_or_else(PathBuf::new, |root| root.join("hooks").join("docs")),
  }
}

fn plugin_root_of(env: &Env, host: Host) -> String {
  let codex = if host == Host::Codex {
    env.get("PLUGIN_ROOT").unwrap_or("")
  } else {
    ""
  };
  if codex.is_empty() {
    env.get("CLAUDE_PLUGIN_ROOT").unwrap_or("").to_owned()
  } else {
    codex.to_owned()
  }
}

fn verbose(env: &Env) -> bool {
  env.get("TOOLU_VERBOSE").is_some_and(|value| value != "0")
}

fn nonempty(top: &str) -> Option<&Path> {
  (!top.is_empty()).then(|| Path::new(top))
}

fn push_present(parts: &mut Vec<String>, part: Option<String>) {
  if let Some(part) = part.filter(|part| !part.is_empty()) {
    parts.push(part);
  }
}

fn hook_json(context: Option<&str>, message: &str) -> String {
  let mut fields = Vec::new();
  if let Some(context) = context {
    fields.push((
      "hookSpecificOutput".to_owned(),
      Ordered::Object(vec![
        (
          "hookEventName".to_owned(),
          Ordered::String("SessionStart".to_owned()),
        ),
        (
          "additionalContext".to_owned(),
          Ordered::String(context.to_owned()),
        ),
      ]),
    ));
  }
  fields.push((
    "systemMessage".to_owned(),
    Ordered::String(message.to_owned()),
  ));
  let rendered = render_hook_output(&Ordered::Object(fields), true);
  strip_trailing_newlines(&rendered).to_owned()
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
