//! GraphQL queries. A reply with `errors[]` is an error; the cost is the
//! `rateLimit { cost }` the query selects.

use serde_json::{Value, json};
use toolu_http::Method;

use crate::call::Call;
use crate::cost::Cost;
use crate::{Client, Error, Reply};

impl Client {
  /// Run `query` with `variables`; the reply's data is the body's `data`.
  /// Select `rateLimit { cost }` to have the cost reported.
  ///
  /// # Errors
  /// `GraphQl` for a reply with `errors[]`, `Decode` for one without `data`,
  /// and the call's failures.
  pub fn graphql(&self, query: &str, variables: &Value) -> Result<Reply<Value>, Error> {
    let body = json!({ "query": query, "variables": variables }).to_string();
    let (response, attempts) = self.call(&Call {
      method: Method::Post,
      path: "/graphql",
      body: Some(body.into_bytes()),
      etag: None,
    })?;
    let mut document: Value =
      serde_json::from_slice(&response.body).map_err(|err| crate::error::decode(&err))?;
    if let Some(messages) = errors(&document) {
      let tokens = self.tokens();
      return Err(Error::GraphQl(
        messages
          .iter()
          .map(|message| tokens.redact(message))
          .collect(),
      ));
    }
    let data = document
      .get_mut("data")
      .map(Value::take)
      .filter(|data| !data.is_null())
      .ok_or_else(|| Error::Decode("the GraphQL reply has no data".into()))?;
    let cost = Cost::graphql(&response, &data);
    Ok(Reply {
      data,
      cost,
      attempts,
    })
  }
}

/// The messages of a non-empty `errors[]`.
pub(crate) fn errors(document: &Value) -> Option<Vec<String>> {
  let errors = document.get("errors")?.as_array()?;
  if errors.is_empty() {
    return None;
  }
  Some(
    errors
      .iter()
      .map(|error| {
        error
          .get("message")
          .and_then(Value::as_str)
          .map_or_else(|| "GraphQL error".to_owned(), str::to_owned)
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/graphql_test.rs"]
mod tests;
