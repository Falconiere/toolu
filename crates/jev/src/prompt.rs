//! The `UserPromptSubmit` hook: repeat the Jev mandate on a substantive prompt. No
//! network call.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

use crate::session::{context_outcome, credential_notice, payload, silent, skill_reference};

static TRIVIAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
  Regex::new(
    r"(?i)^\s*(?:y|n|yes|no|ok|okay|sure|thanks|thank you|go ahead|looks good|lgtm|correct|exactly|right|done|nah|nope|yep|yup|continue)[.!?]?\s*$",
  )
  .ok()
});

fn trivial(prompt: &str) -> bool {
  TRIVIAL
    .as_ref()
    .is_some_and(|pattern| pattern.is_match(prompt))
}

/// The prompt text of a payload that is an object with a non-empty string `prompt`.
fn prompt_of(stdin: Option<&str>) -> Option<String> {
  match payload(stdin)?.get("prompt") {
    Some(Ordered::String(prompt)) if !prompt.is_empty() => Some(prompt.clone()),
    _ => None,
  }
}

/// The reminder for a substantive prompt, or nothing for a confirmation or an
/// unreadable payload.
pub fn user_prompt_submit(env: &Env, plugin_root: &Path, stdin: Option<&str>) -> Outcome {
  if prompt_of(stdin).is_none_or(|prompt| trivial(&prompt)) {
    return silent();
  }
  context_outcome(
    "UserPromptSubmit",
    &format!(
      concat!(
        "{}Jev is mandatory for this task when it contains semantic decisions. After initial ",
        "exploration, identify useful judgments over supplied evidence; you MUST call toolu jev ",
        "before the decision it informs. The command does not load a project .env ",
        "file. Reassess after new evidence, failed hypotheses, or changed requirements. Batch ",
        "independent questions in one ask call. Reuse unchanged evidence and questions rather ",
        "than repeating calls. If a task has no semantic decision, say so in one sentence rather ",
        "than skipping silently. Syntax and linked examples: {}. On failure, state the ",
        "limitation and use an evidence fallback; Jev never replaces tests or authorization."
      ),
      credential_notice(env),
      skill_reference(plugin_root),
    ),
  )
}

#[cfg(test)]
#[path = "tests/prompt_test.rs"]
mod tests;
