//! `toolu hook user-prompt-submit` (#424). A trivial reply or a slash command
//! prints nothing. A one-word verb blocks. Anything else is hints joined by ` | `.

use std::path::{Path, PathBuf};

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::load::{LoadedConfig, is_file, load};
use toolu_runtime::config::read::enabled;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, Wait, run};
use toolu_runtime::startup::context::render_hook_output;

use crate::lifecycle::diagnostics::diagnostics;
use crate::lifecycle::mandates::has_ast_grep;
use crate::lifecycle::project::{git_toplevel, on_path};
use crate::lifecycle::prompt::{
  HintOptions, PromptGate, mentions_gate_topic, prompt_gate, prompt_hints,
};
use crate::lifecycle::text::{ascii_lower, jq_alt, member, parse_stdin, strip_trailing_newlines};

const BLOCK_REASON: &str = "Prompt too vague - specify what file/feature/error needs attention";

/// What the native user-prompt-submit hook reads.
pub struct PromptInput<'a> {
  /// The hook's environment.
  pub env: &'a Env,
  /// The directory the hook was started in.
  pub cwd: &'a Path,
  /// The hook payload. The prompt is `.prompt`.
  pub payload: &'a str,
}

/// Run user-prompt-submit. A disabled hook, an empty prompt, and a trivial reply print nothing.
pub fn prompt_submit(input: &PromptInput<'_>) -> Outcome {
  let detected = detect(input.env, None);
  let roots = Roots::new(input.env.clone(), Some(detected.host));
  let config = load(&roots, Some(input.cwd));
  let stdout = enabled(&config, "hooks", "user-prompt-submit")
    .then(|| reply(input, detected.host, &config))
    .flatten();
  Outcome {
    exit: Exit::Success,
    stdout,
    stderr: diagnostics(detected.warning, &config, Vec::new()),
  }
}

fn reply(input: &PromptInput<'_>, host: Host, config: &LoadedConfig) -> Option<String> {
  let doc = parse_stdin(input.payload);
  let prompt = jq_alt(member(doc.as_ref(), "prompt"), "");
  if prompt.is_empty() {
    return None;
  }
  let lower = ascii_lower(&prompt);
  match prompt_gate(&lower) {
    PromptGate::Skip => None,
    PromptGate::Block => Some(block_json()),
    PromptGate::Hint => Some(hint_json(&additional_context(
      input, host, config, &prompt, &lower,
    ))),
  }
}

fn additional_context(
  input: &PromptInput<'_>,
  host: Host,
  config: &LoadedConfig,
  prompt: &str,
  lower: &str,
) -> String {
  let top = git_toplevel(input.env, input.cwd);
  let root = if top.is_empty() {
    input.cwd.to_path_buf()
  } else {
    PathBuf::from(top)
  };
  let dirname = Roots::new(input.env.clone(), Some(host)).project_dirname();
  let mut parts = prompt_hints(
    lower,
    &HintOptions {
      ast_grep: has_ast_grep(input.env) && enabled(config, "skills", "ast-grep"),
      research: enabled(config, "agents", "research-agent"),
    },
  );
  let project = project_context(
    &root.join(&dirname).join("context.sh"),
    prompt,
    input.cwd,
    input.env,
  );
  if !project.is_empty() {
    parts.push(project);
  }
  if !mentions_gate_topic(lower)
    && let Some(hint) = failing_gate_hint(&root.join(&dirname).join("tmp"))
  {
    parts.push(hint);
  }
  parts.join(" | ")
}

fn failing_gate_hint(state_root: &Path) -> Option<String> {
  let file = state_root.join("quality-gate-status.json");
  if !is_file(&file) {
    return None;
  }
  let text = std::fs::read_to_string(&file).ok()?;
  let doc = parse_stdin(&text);
  let status = member(doc.as_ref(), "status")?.as_str()?;
  if status != "failing" {
    return None;
  }
  let reason = jq_alt(member(doc.as_ref(), "reason"), "Unknown quality failure");
  Some(format!(
    "Quality gate failing: {reason}. Prefer fixing before unrelated work."
  ))
}

fn project_context(script: &Path, prompt: &str, cwd: &Path, env: &Env) -> String {
  if !script.exists() || !is_file(script) || !on_path("bash", env.get("PATH").unwrap_or("")) {
    return String::new();
  }
  let mut spec = Spec::new(["bash"]);
  spec.argv.push(script.display().to_string());
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone().with("PROMPT", prompt));
  spec.wait = Wait::Streams;
  let Ok(output) = run(&spec) else {
    return String::new();
  };
  if output.timed_out {
    return String::new();
  }
  strip_trailing_newlines(&output.stdout).to_owned()
}

fn block_json() -> String {
  hook_json(vec![
    ("decision".to_owned(), Ordered::String("block".to_owned())),
    (
      "reason".to_owned(),
      Ordered::String(BLOCK_REASON.to_owned()),
    ),
  ])
}

fn hint_json(context: &str) -> String {
  let specific = Ordered::Object(vec![
    (
      "hookEventName".to_owned(),
      Ordered::String("UserPromptSubmit".to_owned()),
    ),
    (
      "additionalContext".to_owned(),
      Ordered::String(context.to_owned()),
    ),
  ]);
  hook_json(vec![("hookSpecificOutput".to_owned(), specific)])
}

fn hook_json(fields: Vec<(String, Ordered)>) -> String {
  let rendered = render_hook_output(&Ordered::Object(fields), true);
  strip_trailing_newlines(&rendered).to_owned()
}

#[cfg(test)]
#[path = "tests/prompt_submit_test.rs"]
mod tests;
