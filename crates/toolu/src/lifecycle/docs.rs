//! Session-start titles and `{{token}}` docs (`session-docs.ts`, `render-doc.ts`).

use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::config::read::{CodexModel, MODEL_CLASSES, codex_model, enabled, model};

use crate::lifecycle::project::ProjectFacts;
use crate::lifecycle::text::strip_trailing_newlines;

const TITLES: &[(&str, &str)] = &[
  ("startup", "Toolu is on!"),
  ("resume", "Session resumed"),
  ("clear", "Context cleared"),
  ("compact", "Context compacted"),
];

/// What [`doc_parts`] reads. `docs` is `<plugin root>/hooks/docs`.
pub(crate) struct DocInput<'a> {
  /// The directory of `hooks/docs`.
  pub docs: &'a Path,
  /// The session event (`startup`, `resume`, `clear`, `compact`, …).
  pub event: &'a str,
  /// The merged config.
  pub config: &'a LoadedConfig,
  /// The host the routing table is rendered for.
  pub host: Host,
  /// Project facts. Empty name and package manager take the doc fallbacks.
  pub facts: &'a ProjectFacts,
  /// `TOOLU_VERBOSE` is set and not `0`.
  pub verbose: bool,
}

/// The `systemMessage` title. An event the bash hook did not name is empty.
pub(crate) fn event_title(event: &str) -> &str {
  TITLES
    .iter()
    .find(|(name, _)| *name == event)
    .map_or("", |(_, title)| *title)
}

/// Replace `{{token}}` pairs in order. An unreadable file is an empty part.
pub(crate) fn render_doc(path: &Path, tokens: &[(&str, &str)]) -> String {
  let Ok(text) = std::fs::read_to_string(path) else {
    return String::new();
  };
  let mut content = strip_trailing_newlines(&text).to_owned();
  for (token, value) in tokens {
    let needle = format!("{{{{{token}}}}}");
    content = content.replace(&needle, value);
  }
  content
}

/// The doc parts in session-start.sh's order. Missing docs are skipped.
pub(crate) fn doc_parts(input: &DocInput<'_>) -> Vec<String> {
  let tokens = doc_tokens(input);
  let pairs = token_pairs(&tokens);
  let mut parts = Vec::new();
  if input.event == "compact" {
    parts.push(render_doc(&input.docs.join("post-compaction.md"), &[]));
  }
  if !event_title(input.event).is_empty() {
    parts.push(render_doc(&input.docs.join("session-start.md"), &pairs));
  }
  if enabled(input.config, "models", "enabled") {
    parts.push(render_doc(&input.docs.join(model_doc(input.host)), &pairs));
  }
  if input.verbose {
    append_verbose(&mut parts, input, &pairs);
  }
  parts.retain(|part| !part.is_empty());
  parts
}

fn doc_tokens(input: &DocInput<'_>) -> Vec<(String, String)> {
  let name = if input.facts.name.is_empty() {
    "this project"
  } else {
    input.facts.name.as_str()
  };
  let manager = if input.facts.node_pm.is_empty() {
    "your package manager"
  } else {
    input.facts.node_pm.as_str()
  };
  let mut tokens = vec![
    ("project_name".to_owned(), name.to_owned()),
    ("node_pm".to_owned(), manager.to_owned()),
  ];
  tokens.extend(routing_tokens(input.config, input.host));
  tokens
}

fn routing_tokens(config: &LoadedConfig, host: Host) -> Vec<(String, String)> {
  let mut models = Vec::new();
  let mut efforts = Vec::new();
  for class in MODEL_CLASSES {
    if host == Host::Codex {
      let picked = codex_model(config, class).unwrap_or(CodexModel {
        model: String::new(),
        reasoning_effort: String::new(),
      });
      models.push((format!("model_{class}"), picked.model));
      efforts.push((
        format!("effort_{class}"),
        format!(", effort `{}`", picked.reasoning_effort),
      ));
    } else {
      let name = model(config, class).unwrap_or_else(|_| "inherit".to_owned());
      models.push((format!("model_{class}"), name));
      efforts.push((format!("effort_{class}"), String::new()));
    }
  }
  models.extend(efforts);
  models
}

fn token_pairs(tokens: &[(String, String)]) -> Vec<(&str, &str)> {
  tokens
    .iter()
    .map(|(key, value)| (key.as_str(), value.as_str()))
    .collect()
}

fn model_doc(host: Host) -> &'static str {
  if host == Host::Opencode {
    "model-routing-opencode.md"
  } else {
    "model-routing.md"
  }
}

fn append_verbose(parts: &mut Vec<String>, input: &DocInput<'_>, pairs: &[(&str, &str)]) {
  let files = [
    (input.facts.ts, "session-start-ts.md"),
    (input.facts.rust, "session-start-rust.md"),
    (input.facts.python, "session-start-python.md"),
  ];
  for (include, file) in files {
    if include {
      parts.push(render_doc(&input.docs.join(file), pairs));
    }
  }
}

#[cfg(test)]
#[path = "tests/docs_test.rs"]
mod tests;
