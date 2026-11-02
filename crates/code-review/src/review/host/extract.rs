//! Answer text and usage from host CLI stdout. Exit codes lie (`opencode`
//! exits 0 on auth failure), so error envelopes fail here.

use toolu_protocol::host::Host;

#[cfg(test)]
#[path = "tests/extract_test.rs"]
mod tests;

/// Text and measured usage extracted from a host CLI's stdout.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Extracted {
  /// The model's answer text.
  pub(crate) text: String,
  /// Input tokens, zero when the host does not report them.
  pub(crate) tokens_in: u64,
  /// Output tokens, zero when the host does not report them.
  pub(crate) tokens_out: u64,
  /// Dollars, `None` when the host does not report a cost.
  pub(crate) cost_usd: Option<f64>,
}

/// Parse `stdout` of `host` into answer text and usage.
pub(crate) fn extract(host: Host, stdout: &str) -> Result<Extracted, String> {
  match host {
    Host::Claude => extract_claude(stdout),
    Host::Cursor => extract_cursor(stdout),
    Host::Opencode => extract_opencode(stdout),
    Host::Codex | Host::Muse => extract_raw(host, stdout),
    Host::Hermes => Err("hermes has no review CLI".to_owned()),
  }
}

fn envelope(stdout: &str, host: &str) -> Result<serde_json::Value, String> {
  serde_json::from_str(stdout).map_err(|_err| format!("{host} printed no JSON envelope"))
}

fn extract_claude(stdout: &str) -> Result<Extracted, String> {
  let doc = envelope(stdout, "claude")?;
  if doc.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
    return Err(describe(&doc));
  }
  let text = doc
    .get("result")
    .and_then(serde_json::Value::as_str)
    .ok_or_else(|| "claude printed no result".to_owned())?;
  let usage = doc.get("usage");
  Ok(Extracted {
    text: text.to_owned(),
    tokens_in: usage_tokens(usage, "input_tokens"),
    tokens_out: usage_tokens(usage, "output_tokens"),
    cost_usd: doc
      .get("total_cost_usd")
      .and_then(serde_json::Value::as_f64),
  })
}

fn extract_cursor(stdout: &str) -> Result<Extracted, String> {
  let doc = envelope(stdout, "cursor")?;
  if doc.get("type").and_then(serde_json::Value::as_str) != Some("result") {
    return Err(describe(&doc));
  }
  let text = doc
    .get("result")
    .and_then(serde_json::Value::as_str)
    .ok_or_else(|| "cursor printed no result".to_owned())?;
  let usage = doc.get("usage");
  Ok(Extracted {
    text: text.to_owned(),
    tokens_in: usage_tokens(usage, "inputTokens"),
    tokens_out: usage_tokens(usage, "outputTokens"),
    cost_usd: None,
  })
}

fn extract_opencode(stdout: &str) -> Result<Extracted, String> {
  let doc = envelope(stdout, "opencode")?;
  if doc.get("type").and_then(serde_json::Value::as_str) == Some("error") {
    return Err(describe(&doc));
  }
  match doc.get("result").and_then(serde_json::Value::as_str) {
    Some(text) => Ok(unmeasured(text.to_owned())),
    // The success shape is unverified; let the findings parser decide.
    None if stdout.trim().is_empty() => Err("opencode printed nothing".to_owned()),
    None => Ok(unmeasured(stdout.trim().to_owned())),
  }
}

/// With `--output-schema` stdout is the shaped answer itself.
fn extract_raw(host: Host, stdout: &str) -> Result<Extracted, String> {
  let text = stdout.trim();
  if text.is_empty() {
    return Err(format!("{} printed nothing", host.name()));
  }
  serde_json::from_str::<serde_json::Value>(text)
    .map_err(|_err| format!("{} printed no JSON answer", host.name()))?;
  Ok(unmeasured(text.to_owned()))
}

fn unmeasured(text: String) -> Extracted {
  Extracted {
    text,
    tokens_in: 0,
    tokens_out: 0,
    cost_usd: None,
  }
}

fn usage_tokens(usage: Option<&serde_json::Value>, key: &str) -> u64 {
  usage
    .and_then(|doc| doc.get(key))
    .and_then(serde_json::Value::as_u64)
    .unwrap_or(0)
}

fn describe(doc: &serde_json::Value) -> String {
  for key in ["message", "errors", "error"] {
    if let Some(hit) = doc
      .get(key)
      .or_else(|| doc.get("error").and_then(|err| err.get(key)))
    {
      return hit.to_string().chars().take(160).collect();
    }
  }
  "the host reported an error".to_owned()
}
