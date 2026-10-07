//! REST calls, conditional on the caller's `ETag`: an unchanged resource answers
//! `304` and costs no rate-limit point.

use serde::de::DeserializeOwned;
use serde_json::Value;
use toolu_http::Method;

use crate::call::Call;
use crate::cost::Cost;
use crate::{Client, Error, Reply};

/// One REST request.
#[derive(Debug, Clone, Copy)]
pub struct RestRequest<'a> {
  /// The method.
  pub method: Method,
  /// A path under the API (`/repos/o/r/pulls/1`), or a URL that starts with
  /// the API URL, such as a pagination link.
  pub path: &'a str,
  /// A JSON body.
  pub body: Option<&'a Value>,
  /// The `ETag` of the caller's cached copy, sent as `If-None-Match`.
  pub etag: Option<&'a str>,
}

/// A REST reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rest {
  /// `304`: the caller's copy is current.
  NotModified,
  /// A new representation.
  Fresh(Fresh),
}

/// A REST response that carries a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fresh {
  /// The HTTP status.
  pub status: u16,
  /// The `ETag` to send next time.
  pub etag: Option<String>,
  /// Every response header, names in lowercase.
  pub headers: Vec<(String, String)>,
  /// The body bytes.
  pub body: Vec<u8>,
}

impl Fresh {
  /// The body decoded from JSON.
  ///
  /// # Errors
  /// `Decode` when the body is not `T`.
  pub fn json<T: DeserializeOwned>(&self) -> Result<T, Error> {
    serde_json::from_slice(&self.body).map_err(|err| Error::Decode(err.to_string()))
  }
}

impl Client {
  /// `GET path`, conditional on `etag`.
  ///
  /// # Errors
  /// See [`Client::rest`].
  pub fn get(&self, path: &str, etag: Option<&str>) -> Result<Reply<Rest>, Error> {
    self.rest(&RestRequest {
      method: Method::Get,
      path,
      body: None,
      etag,
    })
  }

  /// Send a REST request with the client's retry policy.
  ///
  /// # Errors
  /// `Config` for a path outside the API, and the call's failures.
  pub fn rest(&self, request: &RestRequest<'_>) -> Result<Reply<Rest>, Error> {
    let body = request
      .body
      .map(serde_json::to_vec)
      .transpose()
      .map_err(|err| Error::Config(format!("cannot encode the request body: {err}")))?;
    let (response, attempts) = self.call(&Call {
      method: request.method,
      path: request.path,
      body,
      etag: request.etag,
    })?;
    let cost = Cost::rest(&response);
    let data = if response.status == 304 {
      Rest::NotModified
    } else {
      Rest::Fresh(Fresh {
        status: response.status,
        etag: response.header("etag").map(str::to_owned),
        headers: response.headers,
        body: response.body,
      })
    };
    Ok(Reply {
      data,
      cost,
      attempts,
    })
  }
}

#[cfg(test)]
#[path = "tests/rest_test.rs"]
mod tests;
