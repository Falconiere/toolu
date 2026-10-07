//! `Client::send`: one request with extra headers, answered with the status,
//! headers and bounded body of every status. Credentials travel only through
//! [`Auth`], which redirects drop, so credential headers are refused here.

use crate::error::{Error, map_http, map_ureq};
use crate::{Auth, Client};

/// Header names `send` refuses: a credential must go through [`Auth`].
const CREDENTIAL_HEADERS: [&str; 3] = ["authorization", "proxy-authorization", "cookie"];

/// An HTTP method `send` supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
  /// `GET`.
  Get,
  /// `POST`.
  Post,
  /// `PUT`.
  Put,
  /// `PATCH`.
  Patch,
  /// `DELETE`.
  Delete,
}

impl Method {
  /// The `http` crate's method.
  pub(crate) fn http(self) -> ureq::http::Method {
    match self {
      Self::Get => ureq::http::Method::GET,
      Self::Post => ureq::http::Method::POST,
      Self::Put => ureq::http::Method::PUT,
      Self::Patch => ureq::http::Method::PATCH,
      Self::Delete => ureq::http::Method::DELETE,
    }
  }
}

/// One request for [`Client::send`].
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
  /// The method.
  pub method: Method,
  /// An absolute `http` or `https` URL.
  pub url: &'a str,
  /// The credential for the first hop.
  pub auth: &'a Auth,
  /// Extra headers; the caller owns `Content-Type`.
  pub headers: &'a [(&'a str, &'a str)],
  /// The body, if any.
  pub body: Option<&'a [u8]>,
}

/// A response of any status, its body bounded by `Config::max_body_bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
  /// The HTTP status.
  pub status: u16,
  /// Header names in lowercase, with their values.
  pub headers: Vec<(String, String)>,
  /// The body bytes.
  pub body: Vec<u8>,
}

impl Response {
  /// The first value of header `name`, compared without case.
  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }
}

impl Client {
  /// Send `request` and return its response whatever the status.
  ///
  /// # Errors
  /// `InvalidConfig` for a credential header, `Transport` for a bad URL or a
  /// network failure, `Timeout` past the deadline, `BodyTooLarge` past the cap.
  pub fn send(&self, request: &Request<'_>) -> Result<Response, Error> {
    if let Some((name, _)) = request.headers.iter().find(|(name, _)| {
      CREDENTIAL_HEADERS
        .iter()
        .any(|refused| name.eq_ignore_ascii_case(refused))
    }) {
      return Err(Error::InvalidConfig(format!(
        "header {name} is refused; credentials go through Auth"
      )));
    }
    let (uri, scheme) = parse(request.url)?;
    let agent = self.agent(&scheme)?;
    let mut builder = ureq::http::Request::builder()
      .method(request.method.http())
      .uri(uri);
    if let Some(value) = request.auth.header_value() {
      builder = builder.header(ureq::http::header::AUTHORIZATION, value);
    }
    for (name, value) in request.headers {
      builder = builder.header(*name, *value);
    }
    let response = match request.body {
      Some(bytes) => agent.run(builder.body(bytes.to_vec()).map_err(|err| map_http(&err))?),
      None => agent.run(builder.body(()).map_err(|err| map_http(&err))?),
    }
    .map_err(|err| map_ureq(&err))?;
    self.read_response(response)
  }
}

/// Check `url` as [`Client::send`] parses it, so a caller can refuse a URL
/// that no attempt could send before it retries anything.
///
/// # Errors
/// `Transport` naming why the URL is unusable: not a URI, or no `http` or
/// `https` scheme and host.
pub fn check_url(url: &str) -> Result<(), Error> {
  parse(url).map(|_| ())
}

/// `url` as a URI, with its scheme.
fn parse(url: &str) -> Result<(ureq::http::Uri, String), Error> {
  let uri: ureq::http::Uri = url
    .parse()
    .map_err(|err: ureq::http::uri::InvalidUri| Error::Transport(format!("invalid URL: {err}")))?;
  let scheme = uri
    .scheme_str()
    .filter(|scheme| matches!(*scheme, "http" | "https") && uri.host().is_some())
    .ok_or_else(|| Error::Transport("URL must have an HTTP(S) scheme and host".into()))?
    .to_owned();
  Ok((uri, scheme))
}

#[cfg(test)]
#[path = "tests/send_test.rs"]
mod tests;
