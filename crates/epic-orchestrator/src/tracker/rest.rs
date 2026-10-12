//! JSON over `toolu-http` for Jira and Linear, under the epic retry policy.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use toolu_http::{Auth, Client, Config, Method, Request};
use toolu_runtime::env::Env;

use crate::retry::{Budget, Policy, http_failure, scoped_budget, with_retry};

/// One service's HTTP client.
pub(crate) struct Http {
  client: Client,
  policy: Policy,
  budget: Arc<Mutex<Budget>>,
  label: &'static str,
  secrets: Vec<String>,
  auth: Auth,
}

impl Http {
  /// A client for `env`, labelled `label` in messages.
  ///
  /// # Errors
  /// A malformed proxy setting.
  pub(crate) fn new(env: &Env, label: &'static str) -> Result<Self, String> {
    let client = Client::new(Config::default(), env).map_err(|err| err.to_string())?;
    Ok(Self::with_client(client, Policy::from_env(env), label))
  }

  /// An existing client; tests pass one that trusts the loopback fixture.
  pub(crate) fn with_client(client: Client, policy: Policy, label: &'static str) -> Self {
    Self {
      client,
      budget: scoped_budget(),
      policy,
      label,
      secrets: Vec::new(),
      auth: Auth::None,
    }
  }

  /// Send `auth` with every request.
  pub(crate) fn authorized(mut self, auth: Auth) -> Self {
    self.auth = auth;
    self
  }

  /// Replace `secret` with `[redacted]` in every error this client returns.
  pub(crate) fn redacting(mut self, secret: &str) -> Self {
    if !secret.is_empty() {
      self.secrets.push(secret.to_owned());
    }
    self
  }

  fn redact(&self, text: String) -> String {
    self.secrets.iter().fold(text, |text, secret| {
      text.replace(secret.as_str(), "[redacted]")
    })
  }

  /// Send `body` as JSON and decode the reply; an empty reply is `null`.
  /// `write` marks a call that may not repeat a transient failure.
  ///
  /// # Errors
  /// The call fails after its retries, or the reply is not JSON.
  pub(crate) fn json(
    &self,
    method: Method,
    url: &str,
    body: Option<&Value>,
    write: bool,
  ) -> Result<Value, String> {
    let encoded = body
      .map(serde_json::to_vec)
      .transpose()
      .map_err(|err| err.to_string())?;
    let headers: &[(&str, &str)] = if encoded.is_some() {
      &[
        ("Accept", "application/json"),
        ("Content-Type", "application/json"),
      ]
    } else {
      &[("Accept", "application/json")]
    };
    let mut budget = self
      .budget
      .lock()
      .map_or(Budget::unbounded(), |guard| *guard);
    let bytes = with_retry(&self.policy, &mut budget, write, || {
      let outcome = self.client.send(&Request {
        method,
        url,
        auth: &self.auth,
        headers,
        body: encoded.as_deref(),
      });
      http_failure(self.label, url, outcome)
    });
    if let Ok(mut current) = self.budget.lock() {
      *current = budget;
    }
    let bytes = bytes.map_err(|err| self.redact(err))?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
      return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes)
      .map_err(|err| format!("{}: invalid JSON reply: {err}", self.label))
  }
}

#[cfg(test)]
#[path = "tests/rest_test.rs"]
mod tests;
