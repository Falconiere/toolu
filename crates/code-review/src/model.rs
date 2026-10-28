//! Strict review finding schema and fail-closed normalization of model text.

use serde_json::Value;
use toolu_openrouter::{
  Completion, Error as OpenRouterError, Message, StructuredRequest, Usage as OpenRouterUsage,
};

use crate::budget::Effort;
use crate::contract::{Assessment, Cost, ErrorKind, Report, Severity, Side, Usage};

mod salvage;

use salvage::salvage;

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
  /// Jev's assessment once the judge stage ran; `None` before it.
  pub assessment: Option<Assessment>,
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
#[derive(Debug, Clone, PartialEq)]
pub enum ModelError {
  /// Invalid JSON or no valid findings to salvage.
  Malformed {
    /// The provider's finish reason, if supplied.
    finish_reason: Option<String>,
    /// Billed usage of the unusable completion.
    usage: Usage,
  },
}

impl ModelError {
  /// Convert unusable model text into a typed `error` verdict that keeps its cost.
  pub fn report(&self, effort: Effort) -> Report {
    let Self::Malformed {
      finish_reason,
      usage,
    } = self;
    Report {
      usage: usage.clone(),
      ..Report::failed(
        effort,
        ErrorKind::Outage,
        &format!(
          "unusable structured review (finish_reason: {})",
          finish_reason.as_deref().unwrap_or("none")
        ),
      )
    }
  }
}

/// Convert a transport or provider error into a typed `error` verdict.
pub fn error_report(error: &OpenRouterError, effort: Effort) -> Report {
  let kind = match error {
    OpenRouterError::Unconfigured
    | OpenRouterError::InvalidKey
    | OpenRouterError::InvalidConfig => ErrorKind::Unconfigured,
    OpenRouterError::Authentication(_)
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
  let mut report = Report::failed(effort, kind, &error.to_string());
  if let Some((observed, model)) = error.usage() {
    report.usage = convert_usage(observed, Some(model.to_owned()));
  }
  report
}

/// Carry observed review-call usage into the report without inventing a cost.
pub fn usage(completion: &Completion) -> Usage {
  convert_usage(&completion.usage, Some(completion.model.clone()))
}

fn convert_usage(observed: &OpenRouterUsage, model: Option<String>) -> Usage {
  Usage {
    tokens_in: observed.prompt_tokens,
    tokens_out: observed.completion_tokens,
    cost_usd: observed.cost_usd.and_then(Cost::usd),
    model,
    calls: Vec::new(),
  }
}

/// Parse model text, retaining only complete and locally valid findings.
///
/// # Errors
/// Returns `Malformed` when there is no structured finding array, or when no
/// valid finding survives a partial answer: salvaged, dropped, extra fields or
/// a finish other than `stop`.
pub fn parse(completion: &Completion) -> Result<ModelOutput, ModelError> {
  let (values, malformed_source, incomplete) =
    match serde_json::from_str::<Value>(&completion.content) {
      Ok(mut doc) => {
        let extra = doc.as_object().is_none_or(|map| map.len() != 1);
        let Some(Value::Array(values)) = doc.get_mut("findings").map(Value::take) else {
          return Err(malformed(completion));
        };
        (values, extra, 0)
      }
      Err(_err) => {
        let Some((values, incomplete)) = salvage(&completion.content) else {
          return Err(malformed(completion));
        };
        (values, true, incomplete)
      }
    };
  let abnormal_finish = completion.finish_reason.as_deref() != Some("stop");
  let mut output = ModelOutput {
    findings: Vec::new(),
    partial: malformed_source || abnormal_finish,
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
  // A partial answer with nothing usable left is never a clean review.
  if output.findings.is_empty() && output.partial {
    return Err(malformed(completion));
  }
  Ok(output)
}

fn malformed(completion: &Completion) -> ModelError {
  ModelError::Malformed {
    finish_reason: completion.finish_reason.clone(),
    usage: usage(completion),
  }
}

fn finding(value: &Value) -> Option<(ModelFinding, bool)> {
  let map = value.as_object()?;
  let (path, path_changed) = trimmed(value, "path")?;
  let (line, line_changed) = parse_line(value, "line")?;
  let (end_line, end_changed) = parse_line(value, "end_line")?;
  if end_line < line {
    return None;
  }
  let (side, side_changed) = side(value.get("side")?)?;
  let (severity, severity_changed) = severity(value.get("severity")?)?;
  // Category feeds the memory fingerprint, so it is trimmed like the path.
  let (category, category_changed) = trimmed(value, "category")?;
  let claim = text(value, "claim")?;
  let quote = text(value, "quote")?;
  let (suggestion, suggestion_changed) = optional(value, "suggestion")?;
  let (prior_id, prior_changed) = optional(value, "prior_id")?;
  let normalized = map.len() != 10
    || [
      line_changed,
      end_changed,
      path_changed,
      category_changed,
      side_changed,
      severity_changed,
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
      assessment: None,
    },
    normalized,
  ))
}

fn text(value: &Value, key: &str) -> Option<String> {
  let raw = value.get(key)?.as_str()?;
  if raw.trim().is_empty() {
    return None;
  }
  Some(raw.to_owned())
}

fn trimmed(value: &Value, key: &str) -> Option<(String, bool)> {
  let raw = text(value, key)?;
  let trimmed = raw.trim();
  Some((trimmed.to_owned(), trimmed != raw))
}

fn optional(value: &Value, key: &str) -> Option<(Option<String>, bool)> {
  match value.get(key) {
    None => Some((None, true)),
    Some(Value::Null) => Some((None, false)),
    Some(Value::String(raw)) if raw.trim().is_empty() => Some((None, true)),
    Some(Value::String(raw)) => Some((Some(raw.to_owned()), false)),
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

#[cfg(test)]
#[path = "tests/model_test.rs"]
mod tests;
