//! Strict review finding schema and fail-closed normalization of model text.

use serde_json::Value;
use toolu_openrouter::{Completion, Error as OpenRouterError, Message, StructuredRequest};

use crate::budget::Effort;
use crate::contract::{Cost, ErrorKind, Report, Severity, Side, Usage};

/// The committed schema sent as `response_format.json_schema.schema`.
pub const SCHEMA_JSON: &str = include_str!("model.schema.json");

/// Build the review call from the shared effort table, never above its ceiling.
pub fn request<'a>(
  messages: &'a [Message<'a>],
  schema: &'a Value,
  effort: Effort,
) -> StructuredRequest<'a> {
  let budget = effort.budget();
  StructuredRequest {
    messages,
    schema_name: "code_review",
    schema,
    reasoning_effort: budget.reasoning,
    max_tokens: budget.max_tokens,
  }
}

/// A finding before #668 grounds it in the diff and assigns an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFinding {
  /// Repository-relative path claimed by the model.
  pub path: String,
  /// First claimed line.
  pub line: u32,
  /// Last claimed line.
  pub end_line: u32,
  /// Which side of the diff the claim concerns.
  pub side: Side,
  /// Claimed severity.
  pub severity: Severity,
  /// Claimed category.
  pub category: String,
  /// Defect claim.
  pub claim: String,
  /// Evidence quote to ground later.
  pub quote: String,
  /// Optional proposed fix.
  pub suggestion: Option<String>,
  /// Identity of an open prior finding, when re-raised.
  pub prior_id: Option<String>,
}

/// Locally valid findings and whether any model text was omitted or normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOutput {
  /// Valid finding objects only.
  pub findings: Vec<ModelFinding>,
  /// `true` for truncation, dropped findings or schema deviations.
  pub partial: bool,
  /// Finding objects dropped because they were invalid or incomplete.
  pub dropped: usize,
}

/// Model text that yielded no usable structured result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
  /// Invalid JSON or no valid findings to salvage.
  Malformed {
    /// The provider's finish reason, if supplied.
    finish_reason: Option<String>,
  },
}

impl ModelError {
  /// Convert unusable model text into a typed `error` verdict.
  pub fn report(&self, effort: Effort) -> Report {
    let Self::Malformed { finish_reason } = self;
    Report::failed(
      effort,
      ErrorKind::Outage,
      &format!("unusable structured review (finish_reason: {finish_reason:?})"),
    )
  }
}

/// Convert a transport or provider error into a typed `error` verdict.
pub fn error_report(error: &OpenRouterError, effort: Effort) -> Report {
  let kind = match error {
    OpenRouterError::Unconfigured | OpenRouterError::InvalidKey => ErrorKind::Unconfigured,
    OpenRouterError::InvalidConfig
    | OpenRouterError::Authentication(_)
    | OpenRouterError::RateLimited { .. }
    | OpenRouterError::NoProvider
    | OpenRouterError::Upstream(_)
    | OpenRouterError::Timeout
    | OpenRouterError::Transport
    | OpenRouterError::BodyTooLarge
    | OpenRouterError::InvalidResponse
    | OpenRouterError::Empty { .. }
    | OpenRouterError::Truncated { .. } => ErrorKind::Outage,
  };
  Report::failed(effort, kind, &error.to_string())
}

/// Carry observed review-call usage into the report without inventing a cost.
pub fn usage(completion: &Completion) -> Usage {
  Usage {
    tokens_in: completion.usage.prompt_tokens,
    tokens_out: completion.usage.completion_tokens,
    cost_usd: completion.usage.cost_usd.and_then(Cost::usd),
    model: Some(completion.model.clone()),
  }
}

/// Parse model text, retaining only complete and locally valid findings.
///
/// # Errors
/// Returns `Malformed` when there is no structured finding array or a
/// truncated body has no valid finding to salvage.
pub fn parse(completion: &Completion) -> Result<ModelOutput, ModelError> {
  let (values, truncated, incomplete) = match serde_json::from_str::<Value>(&completion.content) {
    Ok(doc) => {
      let Some(values) = doc.get("findings").and_then(Value::as_array) else {
        return Err(malformed(completion));
      };
      (
        values.clone(),
        doc.as_object().is_none_or(|map| map.len() != 1),
        0,
      )
    }
    Err(_err) => {
      let Some((values, incomplete)) = salvage(&completion.content) else {
        return Err(malformed(completion));
      };
      (values, true, incomplete)
    }
  };
  let mut output = ModelOutput {
    findings: Vec::new(),
    partial: truncated || completion.finish_reason.as_deref() == Some("length"),
    dropped: incomplete,
  };
  for value in &values {
    match finding(value) {
      Some((parsed, normalized)) => {
        output.findings.push(parsed);
        output.partial |= normalized;
      }
      None => output.dropped += 1,
    }
  }
  output.partial |= output.dropped > 0;
  if output.findings.is_empty()
    && (truncated || completion.finish_reason.as_deref() == Some("length"))
  {
    return Err(malformed(completion));
  }
  Ok(output)
}

fn malformed(completion: &Completion) -> ModelError {
  ModelError::Malformed {
    finish_reason: completion.finish_reason.clone(),
  }
}

fn finding(value: &Value) -> Option<(ModelFinding, bool)> {
  let map = value.as_object()?;
  let (path, path_changed) = text(value, "path")?;
  let (line, line_changed) = parse_line(value, "line")?;
  let (end_line, end_changed) = parse_line(value, "end_line")?;
  if end_line < line {
    return None;
  }
  let (side, side_changed) = side(value.get("side")?)?;
  let (severity, severity_changed) = severity(value.get("severity")?)?;
  let (category, category_changed) = text(value, "category")?;
  let (claim, claim_changed) = text(value, "claim")?;
  let (quote, quote_changed) = text(value, "quote")?;
  let (suggestion, suggestion_changed) = optional(value, "suggestion")?;
  let (prior_id, prior_changed) = optional(value, "prior_id")?;
  let normalized = map.len() != 10
    || [
      path_changed,
      line_changed,
      end_changed,
      side_changed,
      severity_changed,
      category_changed,
      claim_changed,
      quote_changed,
      suggestion_changed,
      prior_changed,
    ]
    .contains(&true);
  Some((
    ModelFinding {
      path,
      line,
      end_line,
      side,
      severity,
      category,
      claim,
      quote,
      suggestion,
      prior_id,
    },
    normalized,
  ))
}

fn text(value: &Value, key: &str) -> Option<(String, bool)> {
  let raw = value.get(key)?.as_str()?;
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return None;
  }
  Some((trimmed.to_owned(), trimmed != raw))
}

fn optional(value: &Value, key: &str) -> Option<(Option<String>, bool)> {
  match value.get(key) {
    None => Some((None, true)),
    Some(Value::Null) => Some((None, false)),
    Some(Value::String(raw)) => {
      let trimmed = raw.trim();
      Some((
        (!trimmed.is_empty()).then(|| trimmed.to_owned()),
        trimmed != raw || trimmed.is_empty(),
      ))
    }
    Some(_) => None,
  }
}

fn parse_line(value: &Value, key: &str) -> Option<(u32, bool)> {
  let raw = value.get(key)?;
  let (number, changed) = match raw {
    Value::Number(number) => (u32::try_from(number.as_u64()?).ok()?, false),
    Value::String(text) if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) => {
      (text.parse::<u32>().ok()?, true)
    }
    Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) | Value::String(_) => {
      return None;
    }
  };
  (number > 0).then_some((number, changed))
}

fn side(value: &Value) -> Option<(Side, bool)> {
  let raw = value.as_str()?;
  let lower = raw.trim().to_ascii_lowercase();
  let parsed = match lower.as_str() {
    "new" => Side::New,
    "old" => Side::Old,
    _ => return None,
  };
  Some((parsed, raw != lower))
}

fn severity(value: &Value) -> Option<(Severity, bool)> {
  let raw = value.as_str()?;
  let lower = raw.trim().to_ascii_lowercase();
  let parsed = match lower.as_str() {
    "blocker" => Severity::Blocker,
    "high" => Severity::High,
    "medium" => Severity::Medium,
    "low" => Severity::Low,
    "nit" => Severity::Nit,
    _ => return None,
  };
  Some((parsed, raw != lower))
}

/// Closed JSON objects inside a truncated `findings` array; trailing fragments
/// count once and never become a finding.
fn salvage(content: &str) -> Option<(Vec<Value>, usize)> {
  if !content.trim_start().starts_with('{') {
    return None;
  }
  let (_, after_key) = content.split_once("\"findings\"")?;
  let (_, after_colon) = after_key.split_once(':')?;
  let array = after_colon.trim_start().strip_prefix('[')?;
  let mut values = Vec::new();
  let mut start = None;
  let mut depth = 0_u32;
  let mut in_string = false;
  let mut escape = false;
  let mut closed = false;
  let mut dropped = 0;
  for (index, ch) in array.char_indices() {
    if in_string {
      if escape {
        escape = false;
      } else if ch == '\\' {
        escape = true;
      } else if ch == '"' {
        in_string = false;
      }
      continue;
    }
    match ch {
      '"' => in_string = true,
      '{' => {
        if depth == 0 {
          start = Some(index);
        }
        depth += 1;
      }
      '}' if depth > 0 => {
        depth -= 1;
        if depth == 0 {
          let parsed = closed_object(array, start?, index + 1);
          dropped += usize::from(parsed.is_none());
          values.extend(parsed);
          start = None;
        }
      }
      ']' if depth == 0 => {
        closed = true;
        break;
      }
      _ => {}
    }
  }
  let incomplete = dropped + usize::from(depth > 0 || !closed);
  Some((values, incomplete))
}

fn closed_object(array: &str, start: usize, end: usize) -> Option<Value> {
  serde_json::from_str(array.get(start..end)?).ok()
}

#[cfg(test)]
#[path = "tests/model_test.rs"]
mod tests;
