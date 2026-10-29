//! Jira's five operations for the epic engine (`scripts/trackers/jira-client.ts`),
//! with credentials from the environment only.

use serde_json::{Value, json};
use toolu_http::{Auth, Method};
use toolu_runtime::env::Env;

use super::jira_comment::comment_matches;
use super::rest::Http;

const FIELDS: [&str; 5] = ["summary", "status", "issuelinks", "description", "labels"];
const SETUP: &str =
  "Jira requires JIRA_BASE_URL and either JIRA_PAT or JIRA_EMAIL with JIRA_API_TOKEN";

/// A Jira REST client.
pub(crate) struct JiraClient {
  /// `JIRA_BASE_URL` without a trailing `/`.
  pub base: String,
  /// `2` or `3`.
  pub version: String,
  http: Http,
}

impl JiraClient {
  /// A client from `JIRA_BASE_URL`, `JIRA_PAT` or `JIRA_EMAIL` + `JIRA_API_TOKEN`,
  /// and `JIRA_API_VERSION` (default `3`), checked before any request.
  ///
  /// # Errors
  /// A missing or malformed setting.
  pub(crate) fn new(env: &Env) -> Result<Self, String> {
    Self::with_http(env, Http::new(env, "epic jira")?)
  }

  /// A client over an existing transport.
  ///
  /// # Errors
  /// A missing or malformed setting.
  pub(crate) fn with_http(env: &Env, http: Http) -> Result<Self, String> {
    let base = env
      .get("JIRA_BASE_URL")
      .map(|url| url.trim_end_matches('/').to_owned())
      .unwrap_or_default();
    let auth = match (
      env.get("JIRA_PAT"),
      env.get("JIRA_EMAIL"),
      env.get("JIRA_API_TOKEN"),
    ) {
      (Some(pat), _, _) => Auth::Bearer(pat.to_owned()),
      (None, Some(email), Some(token)) => Auth::Basic {
        username: email.to_owned(),
        password: token.to_owned(),
      },
      _ => return Err(SETUP.to_owned()),
    };
    if base.is_empty() {
      return Err(SETUP.to_owned());
    }
    let scheme = base
      .split_once("://")
      .map(|(scheme, _)| scheme.to_ascii_lowercase());
    if !matches!(scheme.as_deref(), Some("https" | "http")) {
      return Err("JIRA_BASE_URL must use http or https".to_owned());
    }
    let version = env.get("JIRA_API_VERSION").unwrap_or("3").to_owned();
    if version != "2" && version != "3" {
      return Err("JIRA_API_VERSION must be 2 or 3".to_owned());
    }
    let http = ["JIRA_PAT", "JIRA_API_TOKEN"]
      .iter()
      .filter_map(|name| env.get(name))
      .fold(http, Http::redacting)
      .authorized(auth);
    Ok(Self {
      base,
      version,
      http,
    })
  }

  fn url(&self, suffix: &str) -> String {
    format!("{}/rest/api/{}/{suffix}", self.base, self.version)
  }

  fn call(
    &self,
    method: Method,
    suffix: &str,
    body: Option<&Value>,
    write: bool,
  ) -> Result<Value, String> {
    self.http.json(method, &self.url(suffix), body, write)
  }

  /// The whole issue, as Jira answers it.
  ///
  /// # Errors
  /// The call fails.
  pub(crate) fn raw_issue(&self, key: &str) -> Result<Value, String> {
    self.call(Method::Get, &format!("issue/{}", encode(key)), None, false)
  }

  /// One issue with `fields`.
  ///
  /// # Errors
  /// The call fails or the reply is not an issue.
  pub(crate) fn issue(&self, key: &str, fields: &[&str]) -> Result<Value, String> {
    let selected = if fields.is_empty() {
      String::new()
    } else {
      format!("?fields={}", fields.join(","))
    };
    let issue = self.call(
      Method::Get,
      &format!("issue/{}{selected}", encode(key)),
      None,
      false,
    )?;
    checked_issue(issue)
  }

  /// Every issue `jql` finds, up to 20 pages of 100.
  ///
  /// # Errors
  /// A page fails or is malformed.
  pub(crate) fn search(&self, jql: &str) -> Result<Vec<Value>, String> {
    let mut issues = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..20 {
      let (page, more) = if self.version == "2" {
        self.search_v2(jql, issues.len())?
      } else {
        self.search_v3(jql, token.as_deref())?
      };
      let empty = page.issues.is_empty();
      issues.extend(page.issues);
      match more {
        More::Total(total) if issues.len() < total && !empty => {}
        More::Token(Some(next)) => token = Some(next),
        More::Total(_) | More::Token(None) => break,
      }
    }
    Ok(issues)
  }

  fn search_v2(&self, jql: &str, start: usize) -> Result<(Page, More), String> {
    let body = json!({"jql": jql, "fields": FIELDS, "maxResults": 100, "startAt": start});
    let reply = self.call(Method::Post, "search", Some(&body), false)?;
    let total = reply
      .get("total")
      .and_then(Value::as_u64)
      .and_then(|total| usize::try_from(total).ok())
      .ok_or("invalid Jira search response")?;
    Ok((page(&reply)?, More::Total(total)))
  }

  fn search_v3(&self, jql: &str, token: Option<&str>) -> Result<(Page, More), String> {
    let mut body = json!({"jql": jql, "fields": FIELDS, "maxResults": 100});
    if let (Some(token), Some(map)) = (token, body.as_object_mut()) {
      map.insert("nextPageToken".to_owned(), json!(token));
    }
    let reply = self.call(Method::Post, "search/jql", Some(&body), false)?;
    let next = match reply.get("nextPageToken") {
      Some(Value::String(next)) if !next.is_empty() => Some(next.clone()),
      None | Some(Value::Null | Value::String(_)) => None,
      Some(_) => return Err("invalid Jira search response".to_owned()),
    };
    Ok((page(&reply)?, More::Token(next)))
  }
}

/// The workflow calls: transitions, moves and comments.
impl JiraClient {
  /// The issue's available transitions.
  ///
  /// # Errors
  /// The call fails or the reply is malformed.
  pub(crate) fn transitions(&self, key: &str) -> Result<Vec<Value>, String> {
    let reply = self.call(
      Method::Get,
      &format!("issue/{}/transitions", encode(key)),
      None,
      false,
    )?;
    let rows = reply
      .get("transitions")
      .and_then(Value::as_array)
      .filter(|rows| rows.iter().all(is_transition))
      .ok_or("invalid Jira transitions response")?;
    Ok(rows.clone())
  }

  /// Move the issue through the one transition named `name` (any case).
  ///
  /// # Errors
  /// Zero or several transitions match, or a call fails.
  pub(crate) fn transition(&self, key: &str, name: &str) -> Result<(), String> {
    let matches: Vec<Value> = self
      .transitions(key)?
      .into_iter()
      .filter(|row| {
        row
          .get("name")
          .and_then(Value::as_str)
          .is_some_and(|text| text.eq_ignore_ascii_case(name))
      })
      .collect();
    let [only] = matches.as_slice() else {
      return Err(format!(
        "Jira transition \"{name}\" matched {} transitions",
        matches.len()
      ));
    };
    let body = json!({"transition": {"id": only.get("id").cloned().unwrap_or(Value::Null)}});
    self
      .call(
        Method::Post,
        &format!("issue/{}/transitions", encode(key)),
        Some(&body),
        true,
      )
      .map(|_| ())
  }

  /// Comment `text`: plain on v2, a one-paragraph ADF document on v3.
  ///
  /// # Errors
  /// The call fails.
  pub(crate) fn comment(&self, key: &str, text: &str) -> Result<(), String> {
    let content = if self.version == "2" {
      json!(text)
    } else {
      json!({"type": "doc", "version": 1, "content": [
        {"type": "paragraph", "content": [{"type": "text", "text": text}]}
      ]})
    };
    let body = json!({"body": content});
    self
      .call(
        Method::Post,
        &format!("issue/{}/comment", encode(key)),
        Some(&body),
        true,
      )
      .map(|_| ())
  }

  /// Whether a complete comment listing contains the exact original note.
  ///
  /// # Errors
  /// A page fails, is malformed, or cannot be fully traversed.
  pub(crate) fn has_comment(&self, key: &str, note: &str) -> Result<bool, String> {
    let mut start = 0_u64;
    for _ in 0..100 {
      let reply = self.call(
        Method::Get,
        &format!(
          "issue/{}/comment?startAt={start}&maxResults=100",
          encode(key)
        ),
        None,
        false,
      )?;
      let rows = reply
        .get("comments")
        .and_then(Value::as_array)
        .ok_or("invalid Jira comments response")?;
      let total = reply
        .get("total")
        .and_then(Value::as_u64)
        .ok_or("invalid Jira comments response")?;
      if reply.get("startAt").and_then(Value::as_u64) != Some(start)
        || start + rows.len() as u64 > total
      {
        return Err("invalid Jira comments response".to_owned());
      }
      let found = rows.iter().try_fold(false, |found, row| {
        Ok::<bool, String>(found | comment_matches(row, note)?)
      })?;
      if found {
        return Ok(true);
      }
      start += rows.len() as u64;
      if start == total {
        return Ok(false);
      }
      if rows.is_empty() {
        return Err("incomplete Jira comments response".to_owned());
      }
    }
    Err("Jira comments exceed 100 pages".to_owned())
  }
}

struct Page {
  issues: Vec<Value>,
}

enum More {
  Total(usize),
  Token(Option<String>),
}

fn page(reply: &Value) -> Result<Page, String> {
  let issues = reply
    .get("issues")
    .and_then(Value::as_array)
    .ok_or("invalid Jira search response")?;
  if !issues.iter().all(is_issue) {
    return Err("invalid Jira search response".to_owned());
  }
  Ok(Page {
    issues: issues.clone(),
  })
}

/// An issue with a string `key` and an object `fields`.
fn checked_issue(issue: Value) -> Result<Value, String> {
  if is_issue(&issue) {
    Ok(issue)
  } else {
    Err("invalid Jira issue response".to_owned())
  }
}

fn is_issue(issue: &Value) -> bool {
  issue.get("key").is_some_and(Value::is_string)
    && issue.get("fields").is_some_and(Value::is_object)
    && issue
      .pointer("/fields/summary")
      .is_none_or(Value::is_string)
    && issue.pointer("/fields/labels").is_none_or(|labels| {
      labels
        .as_array()
        .is_some_and(|rows| rows.iter().all(Value::is_string))
    })
}

fn is_transition(row: &Value) -> bool {
  row.get("id").is_some_and(Value::is_string) && row.get("name").is_some_and(Value::is_string)
}

/// `encodeURIComponent` for a Jira key.
fn encode(key: &str) -> String {
  key
    .bytes()
    .map(|byte| {
      if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
        char::from(byte).to_string()
      } else {
        format!("%{byte:02X}")
      }
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/jira_client_test.rs"]
mod tests;
