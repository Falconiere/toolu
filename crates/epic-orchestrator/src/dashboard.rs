//! Embedded, same-origin dashboard assets.

/// The exact HTML bytes sent at the root route.
pub(crate) const PAGE: &[u8] = include_bytes!("../assets/dashboard/index.html");
/// The versioned path prefix for immutable assets.
pub(crate) const ASSET_PREFIX: &str = concat!("/assets/", env!("CARGO_PKG_VERSION"), "/");

/// An embedded asset and its HTTP media type.
pub(crate) struct Asset {
  pub bytes: &'static [u8],
  pub content_type: &'static str,
}

/// Find a public, versioned dashboard asset.
pub(crate) fn asset(path: &str) -> Option<Asset> {
  let file = path.strip_prefix(ASSET_PREFIX)?;
  let (bytes, content_type): (&'static [u8], &'static str) = match file {
    "tokens.css" => (include_bytes!("../assets/dashboard/tokens.css"), "text/css"),
    "board.css" => (include_bytes!("../assets/dashboard/board.css"), "text/css"),
    "js/app.js" => (
      include_bytes!("../assets/dashboard/js/app.js"),
      "text/javascript",
    ),
    "js/api.js" => (
      include_bytes!("../assets/dashboard/js/api.js"),
      "text/javascript",
    ),
    "js/logic/status.js" => (
      include_bytes!("../assets/dashboard/js/logic/status.js"),
      "text/javascript",
    ),
    "js/view/dashboard.js" => (
      include_bytes!("../assets/dashboard/js/view/dashboard.js"),
      "text/javascript",
    ),
    "fonts/archivo.woff2" => (
      include_bytes!("../assets/dashboard/fonts/archivo.woff2"),
      "font/woff2",
    ),
    "fonts/jetbrains-mono.woff2" => (
      include_bytes!("../assets/dashboard/fonts/jetbrains-mono.woff2"),
      "font/woff2",
    ),
    _ => return None,
  };
  Some(Asset {
    bytes,
    content_type,
  })
}

#[cfg(test)]
#[path = "tests/dashboard_test.rs"]
mod tests;
