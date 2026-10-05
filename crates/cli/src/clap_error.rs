//! What a clap parse "error" becomes (#442): `--help` and `--version` are data
//! (exit 0), every other kind is a usage error (exit 64) with clap's message on
//! stderr, and under `--json` each one is also a single JSON document on stdout.

use clap::error::{ContextKind, ContextValue, ErrorKind};
use serde_json::json;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

use crate::VERSION;

/// The outcome of `err`; `json` says whether the argv asked for `--json`.
pub(crate) fn outcome(err: &clap::Error, json: bool) -> Outcome {
  let text = err.render().to_string();
  let text = text.trim_end();
  let kind = err.kind();
  if kind == ErrorKind::DisplayHelp {
    return Outcome::data(if json {
      json!({ "help": text }).to_string()
    } else {
      text.to_owned()
    });
  }
  if kind == ErrorKind::DisplayVersion {
    return Outcome::data(if json {
      json!({ "name": "toolu", "version": VERSION, "hookProtocol": HOOK_PROTOCOL }).to_string()
    } else {
      text.to_owned()
    });
  }
  let suggestion = suggestion(err);
  Outcome {
    exit: Exit::Usage,
    stdout: json.then(|| envelope(Exit::Usage, first_line(text), suggestion.as_deref())),
    stderr: Some(text.to_owned()),
  }
}

/// The `error` JSON document.
pub(crate) fn envelope(exit: Exit, message: &str, suggestion: Option<&str>) -> String {
  json!({
    "error": {
      "code": exit.code(),
      "name": exit.name(),
      "message": message,
      "suggestion": suggestion,
    }
  })
  .to_string()
}

/// The first line of `text`, without clap's `error: ` prefix.
pub(crate) fn first_line(text: &str) -> &str {
  let line = text.lines().next().unwrap_or_default();
  line.strip_prefix("error: ").unwrap_or(line)
}

/// The command, flag or value clap suggests in its `tip:` line.
fn suggestion(err: &clap::Error) -> Option<String> {
  [
    ContextKind::SuggestedSubcommand,
    ContextKind::SuggestedArg,
    ContextKind::SuggestedValue,
  ]
  .into_iter()
  .find_map(|kind| {
    let value = err.get(kind)?;
    if let ContextValue::String(one) = value {
      Some(one.clone())
    } else if let ContextValue::Strings(many) = value {
      many.first().cloned()
    } else {
      None
    }
  })
}

#[cfg(test)]
#[path = "tests/clap_error_test.rs"]
mod tests;
