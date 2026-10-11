//! Paginated GitHub reads for one collection: GraphQL pages by `endCursor`,
//! REST pages by the `Link: rel="next"` header, the GraphQL points they
//! cost, and every failure as a structured `api_error`.

use serde_json::{Value, json};
use toolu_engine::babysit::GraphQlUsage;
use toolu_github::{Client, Error, Reply, Rest};

use crate::error::{Failure, Result};

/// A client and the GraphQL budget its reads have spent so far.
pub(crate) struct Reader<'a> {
  client: &'a Client,
  /// The GraphQL points and latest limits observed.
  pub(crate) usage: GraphQlUsage,
}

impl<'a> Reader<'a> {
  /// A reader that has spent nothing yet.
  pub(crate) fn new(client: &'a Client) -> Self {
    Reader {
      client,
      usage: GraphQlUsage {
        points: 0,
        remaining: None,
        reset_at: None,
      },
    }
  }

  /// One GraphQL query's `data`.
  pub(crate) fn graphql(&mut self, source: &str, query: &str, variables: &Value) -> Result<Value> {
    let Reply { data, cost, .. } = self
      .client
      .graphql(query, variables)
      .map_err(|err| failure(source, &err))?;
    self.usage.points += cost.points.unwrap_or(0);
    self.usage.remaining = cost.rate.remaining.or(self.usage.remaining);
    self.usage.reset_at = cost.rate.reset.or(self.usage.reset_at);
    Ok(data)
  }

  /// Every page of a cursor-paginated GraphQL connection at `connection`,
  /// starting after `cursor`.
  pub(crate) fn graphql_pages(
    &mut self,
    source: &str,
    query: &str,
    variables: &Value,
    connection: &str,
  ) -> Result<Vec<Value>> {
    let mut variables = variables.clone();
    let mut pages = Vec::new();
    loop {
      let page = self.graphql(source, query, &variables)?;
      let info = page.pointer(&format!("{connection}/pageInfo")).cloned();
      pages.push(page);
      let next = info.as_ref().and_then(|info| info.get("hasNextPage")) == Some(&Value::Bool(true));
      let cursor = info.and_then(|info| info.get("endCursor").cloned());
      match (next, cursor, variables.as_object_mut()) {
        (true, Some(cursor), Some(object)) if !cursor.is_null() => {
          object.insert("endCursor".into(), cursor);
        }
        _ => return Ok(pages),
      }
    }
  }

  /// Every page of a REST list, following `Link: rel="next"`.
  pub(crate) fn rest_pages(&self, source: &str, path: &str) -> Result<Vec<Value>> {
    let mut pages = Vec::new();
    let mut next = Some(path.to_owned());
    while let Some(path) = next {
      let reply = self
        .client
        .get(&path, None)
        .map_err(|err| failure(source, &err))?;
      let Rest::Fresh(fresh) = reply.data else {
        return Err(invalid(source, "an unconditional read answered 304"));
      };
      pages.push(fresh.json::<Value>().map_err(|err| failure(source, &err))?);
      next = link_next(&fresh.headers);
    }
    Ok(pages)
  }

  /// The pull request's head SHA, over REST.
  pub(crate) fn head(&self, repo: &str, number: u64) -> Result<String> {
    let path = format!("/repos/{repo}/pulls/{number}");
    let pages = self.rest_pages("head", &path)?;
    pages
      .first()
      .and_then(|pr| pr.pointer("/head/sha"))
      .and_then(Value::as_str)
      .filter(|sha| !sha.is_empty())
      .map(str::to_owned)
      .ok_or_else(|| invalid("head", "head read returned no head.sha"))
  }
}

/// The path and query of the `rel="next"` URL in a `Link` header.
pub(crate) fn link_next(headers: &[(String, String)]) -> Option<String> {
  let link = headers
    .iter()
    .find(|(name, _)| name.eq_ignore_ascii_case("link"))
    .map(|(_, value)| value)?;
  let target = link.split(',').find(|part| part.contains("rel=\"next\""))?;
  let url = target.split_once('<')?.1.split_once('>')?.0;
  let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
  after_scheme
    .find('/')
    .and_then(|start| after_scheme.get(start..))
    .map(str::to_owned)
}

fn invalid(source: &str, message: &str) -> Failure {
  Failure::new("invalid_json", format!("read '{source}' failed: {message}"))
    .with("source", source)
    .with("class", "invalid_json")
    .with("lastMessage", message)
}

/// A toolu-github failure as the verb's structured error.
pub(crate) fn failure(source: &str, err: &Error) -> Failure {
  let class = match err {
    Error::Token(_) => {
      return Failure::new("gh_unavailable", format!("no GitHub token: {err}"))
        .with("source", source);
    }
    Error::Config(_) => return Failure::usage(err.to_string()),
    Error::Decode(_) => return invalid(source, &err.to_string()),
    Error::Unauthorized => "unauthorized",
    Error::RateLimited { .. } => "rate_limited",
    Error::Status { status, .. } if *status >= 500 => "transient",
    Error::Status { .. } => "permanent",
    Error::GraphQl(_) => "graphql",
    Error::Transport(_) => "transient",
  };
  let mut failure = Failure::new("api_error", format!("read '{source}' failed: {err}"))
    .with("source", source)
    .with("class", class)
    .with("lastMessage", err.to_string());
  if let Error::RateLimited {
    rate, retry_after, ..
  } = err
  {
    failure = failure
      .with("resetAt", json!(rate.reset))
      .with("remaining", json!(rate.remaining))
      .with("retryAfter", json!(retry_after.map(|wait| wait.as_secs())));
  }
  failure
}

#[cfg(test)]
#[path = "tests/read_test.rs"]
mod tests;
