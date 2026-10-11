//! Host and bearer boundaries for the status listener.

use std::net::SocketAddr;

use toolu_runtime::config::secrets::Secrets;

/// Accept only this socket's address or local aliases on loopback.
pub(crate) fn host_allowed(host: &str, local: SocketAddr) -> bool {
  host.eq_ignore_ascii_case(&local.to_string())
    || (local.ip().is_loopback()
      && (host.eq_ignore_ascii_case(&format!("localhost:{}", local.port()))
        || host.eq_ignore_ascii_case(&format!("127.0.0.1:{}", local.port()))))
}

/// Constant-time comparison for a resolved bearer token.
pub(crate) fn authorized(secrets: &Secrets, header: Option<&str>) -> bool {
  let Some(expected) = secrets.status_token() else {
    return false;
  };
  let Some(given) = header.and_then(|header| header.strip_prefix("Bearer ")) else {
    return false;
  };
  let mut difference = expected.len() ^ given.len();
  for (index, byte) in expected.bytes().enumerate() {
    difference |= usize::from(byte ^ given.as_bytes().get(index).copied().unwrap_or(0));
  }
  difference == 0
}

#[cfg(test)]
#[path = "tests/http_access_test.rs"]
mod tests;
