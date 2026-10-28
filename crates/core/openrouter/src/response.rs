//! `OpenRouter`'s non-streaming response envelope and observed usage.

use std::fmt;

use serde_json::Value;

use crate::Error;

/// Tokens and cost observed in a provider response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Usage {
  /// Input tokens.
  pub prompt_tokens: u64,
  /// Completion tokens, reasoning included.
  pub completion_tokens: u64,
  /// Reasoning tokens, zero when details are absent.
  pub reasoning_tokens: u64,
  /// US dollars, `None` when `OpenRouter` did not supply a valid cost.
  pub cost_usd: Option<f64>,
}

/// A successful non-streaming completion.
#[derive(Clone, PartialEq)]
pub struct Completion {
  /// Visible text, never reasoning text.
  pub content: String,
  /// `OpenRouter`'s finish reason, if supplied.
  pub finish_reason: Option<String>,
  /// The requested model id.
  pub model: String,
  /// The serving provider, unknown when absent.
  pub provider: Option<String>,
  /// Observed usage and optional cost.
  pub usage: Usage,
}

impl fmt::Debug for Completion {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Completion")
      .field("finish_reason", &self.finish_reason)
      .field("model", &self.model)
      .field("provider", &self.provider)
      .field("usage", &self.usage)
      .finish_non_exhaustive()
  }
}

/// Decode a bounded response without echoing provider text in errors.
pub(crate) fn parse(body: &[u8], model: &str) -> Result<Completion, Error> {
  let doc: Value = serde_json::from_slice(body).map_err(|_err| Error::InvalidResponse)?;
  if let Some(error) = doc.get("error") {
    return Err(embedded(error));
  }
  let choice = doc
    .get("choices")
    .and_then(Value::as_array)
    .and_then(|choices| choices.first())
    .ok_or(Error::InvalidResponse)?;
  if let Some(error) = choice.get("error") {
    return Err(embedded(error));
  }
  let finish_reason = choice
    .get("finish_reason")
    .and_then(Value::as_str)
    .map(str::to_owned);
  if finish_reason.as_deref() == Some("error") {
    return Err(Error::Upstream(502));
  }
  let content = choice
    .get("message")
    .and_then(|message| message.get("content"))
    .and_then(Value::as_str)
    .ok_or(Error::InvalidResponse)?;
  if content.trim().is_empty() && finish_reason.as_deref() == Some("length") {
    return Err(Error::Truncated { finish_reason });
  }
  if content.trim().is_empty() && finish_reason.as_deref() != Some("length") {
    return Err(Error::Empty { finish_reason });
  }
  let usage = usage(doc.get("usage"))?;
  if finish_reason.as_deref() == Some("length")
    && usage
      .completion_tokens
      .saturating_sub(usage.reasoning_tokens)
      <= 2
  {
    return Err(Error::Truncated { finish_reason });
  }
  Ok(Completion {
    content: content.to_owned(),
    finish_reason,
    model: model.to_owned(),
    provider: doc
      .get("provider")
      .and_then(Value::as_str)
      .map(str::to_owned),
    usage,
  })
}

fn embedded(error: &Value) -> Error {
  if error
    .get("message")
    .and_then(Value::as_str)
    .is_some_and(|message| {
      let message = message.to_ascii_lowercase();
      message.contains("no endpoints") || message.contains("no provider")
    })
  {
    return Error::NoProvider;
  }
  let code = error
    .get("code")
    .and_then(Value::as_u64)
    .and_then(|code| u16::try_from(code).ok())
    .unwrap_or(502);
  match code {
    401 | 403 => Error::Authentication(code),
    429 => Error::RateLimited {
      retry_after_secs: None,
    },
    _ => Error::Upstream(code),
  }
}

fn usage(value: Option<&Value>) -> Result<Usage, Error> {
  let value = value.ok_or(Error::InvalidResponse)?;
  let prompt_tokens = value
    .get("prompt_tokens")
    .and_then(Value::as_u64)
    .ok_or(Error::InvalidResponse)?;
  let completion_tokens = value
    .get("completion_tokens")
    .and_then(Value::as_u64)
    .ok_or(Error::InvalidResponse)?;
  let reasoning_tokens = value
    .get("completion_tokens_details")
    .and_then(|details| details.get("reasoning_tokens"))
    .and_then(Value::as_u64)
    .unwrap_or(0);
  let cost_usd = value
    .get("cost")
    .and_then(Value::as_f64)
    .filter(|cost| cost.is_finite() && *cost >= 0.0);
  Ok(Usage {
    prompt_tokens,
    completion_tokens,
    reasoning_tokens,
    cost_usd,
  })
}

#[cfg(test)]
#[path = "tests/response_test.rs"]
mod tests;
