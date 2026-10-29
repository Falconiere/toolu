//! GitHub calls for the trackers and the gate: one `toolu-github` client with
//! `Config::scheduled()` (one attempt per call) under the epic retry policy.

use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde_json::Value;
use toolu_github::{Client, Config, Method, Rest, RestRequest};
use toolu_runtime::env::Env;

use crate::retry::{Budget, Policy, github_failure, scoped_budget, with_retry};

/// One reply body and its `Link: rel="next"` page, if any.
type Page = (Vec<u8>, Option<String>);

/// A GitHub client and the retry policy its calls follow.
pub(crate) struct Gh {
  client: Client,
  policy: Policy,
  budget: Arc<Mutex<Budget>>,
}

impl Gh {
  /// A client for `env` with the CLI retry policy.
  ///
  /// # Errors
  /// No GitHub token, or a bad setting.
  pub(crate) fn new(env: &Env) -> Result<Self, String> {
    let client = Client::new(Config::scheduled(), env).map_err(|err| err.to_string())?;
    Ok(Self::with_client(client, Policy::from_env(env)))
  }

  /// An existing client under `policy`, sharing the running effect's sleep
  /// budget (unbounded outside an effect).
  pub(crate) fn with_client(client: Client, policy: Policy) -> Self {
    Self {
      client,
      budget: scoped_budget(),
      policy,
    }
  }

  fn retry<T>(
    &self,
    write: bool,
    call: impl FnMut() -> Result<T, crate::retry::Failure>,
  ) -> Result<T, String> {
    let mut budget = self
      .budget
      .lock()
      .map_or(Budget::unbounded(), |guard| *guard);
    let out = with_retry(&self.policy, &mut budget, write, call);
    if let Ok(mut current) = self.budget.lock() {
      *current = budget;
    }
    out
  }

  /// `GET path` decoded as `T`.
  ///
  /// # Errors
  /// The call fails after its retries, or the body is not `T`.
  pub(crate) fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
    let (body, _) = self.get_page(path)?;
    serde_json::from_slice(&body).map_err(|err| format!("GitHub {path}: {err}"))
  }

  /// Every page of a list endpoint, following `Link: rel="next"`.
  ///
  /// # Errors
  /// A page fails, or a page is not a JSON array.
  pub(crate) fn get_all(&self, path: &str) -> Result<Vec<Value>, String> {
    let mut items = Vec::new();
    let mut next = Some(path.to_owned());
    let mut pages = 0;
    while let Some(page) = next.take() {
      let (body, link) = self.get_page(&page)?;
      let rows: Vec<Value> =
        serde_json::from_slice(&body).map_err(|err| format!("GitHub {page}: {err}"))?;
      items.extend(rows);
      pages += 1;
      if pages == 100 && link.is_some() {
        return Err(format!("GitHub {path}: too many pages"));
      }
      next = link;
    }
    Ok(items)
  }

  /// `GET path` decoded as `T`, or `None` when GitHub answers that the
  /// resource is absent (404, 410) or closed to this token (a 403 that is no
  /// rate limit). A rate limit or a spent sleep budget is still an error.
  ///
  /// # Errors
  /// The call fails after its retries, or the body is not `T`.
  pub(crate) fn get_if_present<T: DeserializeOwned>(
    &self,
    path: &str,
  ) -> Result<Option<T>, String> {
    let Some((body, _)) = self.fetch(path, true)? else {
      return Ok(None);
    };
    serde_json::from_slice(&body).map_err(|err| format!("GitHub {path}: {err}"))
  }

  fn get_page(&self, path: &str) -> Result<Page, String> {
    self.fetch(path, false).map(Option::unwrap_or_default)
  }

  fn fetch(&self, path: &str, absent_ok: bool) -> Result<Option<Page>, String> {
    self.retry(false, || match self.client.get(path, None) {
      Ok(reply) => match reply.data {
        Rest::Fresh(fresh) => {
          let link = next_link(&fresh.headers);
          Ok(Some((fresh.body, link)))
        }
        Rest::NotModified => Ok(Some((b"null".to_vec(), None))),
      },
      Err(toolu_github::Error::Status {
        status: 403 | 404 | 410,
        ..
      }) if absent_ok => Ok(None),
      Err(err) => Err(github_failure(&err)),
    })
  }

  /// A REST write (`PATCH`, `POST`, `PUT`, `DELETE`); a transient failure is not repeated.
  ///
  /// # Errors
  /// The call fails.
  pub(crate) fn write(&self, method: Method, path: &str, body: &Value) -> Result<Value, String> {
    self.retry(true, || {
      let request = RestRequest {
        method,
        path,
        body: Some(body),
        etag: None,
      };
      match self.client.rest(&request) {
        Ok(reply) => Ok(match reply.data {
          Rest::Fresh(fresh) => serde_json::from_slice(&fresh.body).unwrap_or(Value::Null),
          Rest::NotModified => Value::Null,
        }),
        Err(err) => Err(github_failure(&err)),
      }
    })
  }

  /// One REST write with no retry that keeps GitHub's own error: a merge
  /// needs the refusal text, and whether a timeout left it uncertain.
  ///
  /// # Errors
  /// The call fails.
  pub(crate) fn write_once(
    &self,
    method: Method,
    path: &str,
    body: &Value,
  ) -> Result<Value, toolu_github::Error> {
    let request = RestRequest {
      method,
      path,
      body: Some(body),
      etag: None,
    };
    self.client.rest(&request).map(|reply| match reply.data {
      Rest::Fresh(fresh) => serde_json::from_slice(&fresh.body).unwrap_or(Value::Null),
      Rest::NotModified => Value::Null,
    })
  }

  /// A GraphQL query's `data`; `write` marks a mutation.
  ///
  /// # Errors
  /// The call fails or answers `errors[]`.
  pub(crate) fn graphql(
    &self,
    query: &str,
    variables: &Value,
    write: bool,
  ) -> Result<Value, String> {
    self.retry(write, || {
      self
        .client
        .graphql(query, variables)
        .map(|reply| reply.data)
        .map_err(|err| github_failure(&err))
    })
  }
}

/// The `rel="next"` URL of a `Link` header.
fn next_link(headers: &[(String, String)]) -> Option<String> {
  let link = headers
    .iter()
    .find(|(name, _)| name == "link")
    .map(|(_, value)| value.as_str())?;
  link.split(',').find_map(|part| {
    let (url, rel) = part.split_once(';')?;
    rel.contains("rel=\"next\"").then(|| {
      url
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_owned()
    })
  })
}

#[cfg(test)]
#[path = "tests/gh_test.rs"]
mod tests;
