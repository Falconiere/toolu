//! Strict structured-output chat payload, with review routing fixed in code.

use serde_json::{Value, json};

use crate::Error;

/// One chat message supplied by the caller.
#[derive(Debug, Clone, Copy)]
pub struct Message<'a> {
  /// `system`, `developer`, `user`, or `assistant`.
  pub role: &'a str,
  /// Message text.
  pub content: &'a str,
}

/// A non-streaming strict JSON-schema request.
#[derive(Debug)]
pub struct StructuredRequest<'a> {
  /// Ordered chat messages.
  pub messages: &'a [Message<'a>],
  /// Short schema name sent to `OpenRouter`.
  pub schema_name: &'a str,
  /// Committed JSON Schema value owned by the caller.
  pub schema: &'a Value,
  /// The run's model reasoning effort.
  pub reasoning_effort: &'a str,
  /// Combined reasoning and visible output cap.
  pub max_tokens: u32,
}

impl StructuredRequest<'_> {
  /// Encode the fixed strict-schema request after validating its public fields.
  pub(crate) fn encode(&self, model: &str) -> Result<Vec<u8>, Error> {
    if self.messages.is_empty()
      || self.schema_name.is_empty()
      || self.max_tokens == 0
      || !matches!(self.reasoning_effort, "low" | "medium" | "high" | "xhigh")
      || self
        .messages
        .iter()
        .any(|message| !matches!(message.role, "system" | "developer" | "user" | "assistant"))
    {
      return Err(Error::InvalidConfig);
    }
    let messages: Vec<Value> = self
      .messages
      .iter()
      .map(|message| json!({"role":message.role,"content":message.content}))
      .collect();
    let body = json!({
      "model": model,
      "messages": messages,
      "stream": false,
      "max_tokens": self.max_tokens,
      "response_format": {"type":"json_schema","json_schema":{"name":self.schema_name,"strict":true,"schema":self.schema}},
      "reasoning": {"effort":self.reasoning_effort,"exclude":true},
      "provider": {"require_parameters":true,"zdr":true,"quantizations":["fp8","bf16","fp16"]}
    });
    serde_json::to_vec(&body).map_err(|_err| Error::InvalidConfig)
  }
}

#[cfg(test)]
#[path = "tests/request_test.rs"]
mod tests;
