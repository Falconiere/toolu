//! Embedded, same-origin dashboard assets.

/// The exact HTML bytes sent at the root route.
pub(crate) const PAGE: &[u8] = include_bytes!("../assets/dashboard/index.html");
/// The versioned path prefix for immutable assets.
pub(crate) const ASSET_PREFIX: &str = concat!("/assets/", env!("CARGO_PKG_VERSION"), "/");

const CSS: &str = "text/css";
const JS: &str = "text/javascript";
const FONT: &str = "font/woff2";

/// Every served asset: its path under [`ASSET_PREFIX`], bytes and media type.
const ASSETS: [(&str, &[u8], &str); 25] = [
  (
    "tokens.css",
    include_bytes!("../assets/dashboard/tokens.css"),
    CSS,
  ),
  (
    "board.css",
    include_bytes!("../assets/dashboard/board.css"),
    CSS,
  ),
  (
    "board-layout.css",
    include_bytes!("../assets/dashboard/board-layout.css"),
    CSS,
  ),
  (
    "panels.css",
    include_bytes!("../assets/dashboard/panels.css"),
    CSS,
  ),
  (
    "drawer.css",
    include_bytes!("../assets/dashboard/drawer.css"),
    CSS,
  ),
  (
    "control.css",
    include_bytes!("../assets/dashboard/control.css"),
    CSS,
  ),
  (
    "logs.css",
    include_bytes!("../assets/dashboard/logs.css"),
    CSS,
  ),
  (
    "js/app.js",
    include_bytes!("../assets/dashboard/js/app.js"),
    JS,
  ),
  (
    "js/api.js",
    include_bytes!("../assets/dashboard/js/api.js"),
    JS,
  ),
  (
    "js/worker-drawer.js",
    include_bytes!("../assets/dashboard/js/worker-drawer.js"),
    JS,
  ),
  (
    "js/control.js",
    include_bytes!("../assets/dashboard/js/control.js"),
    JS,
  ),
  (
    "js/answer-dialog.js",
    include_bytes!("../assets/dashboard/js/answer-dialog.js"),
    JS,
  ),
  (
    "js/logic/control.js",
    include_bytes!("../assets/dashboard/js/logic/control.js"),
    JS,
  ),
  (
    "js/logic/drawer.js",
    include_bytes!("../assets/dashboard/js/logic/drawer.js"),
    JS,
  ),
  (
    "js/logic/fragment.js",
    include_bytes!("../assets/dashboard/js/logic/fragment.js"),
    JS,
  ),
  (
    "js/logic/logs.js",
    include_bytes!("../assets/dashboard/js/logic/logs.js"),
    JS,
  ),
  (
    "js/logic/status.js",
    include_bytes!("../assets/dashboard/js/logic/status.js"),
    JS,
  ),
  (
    "js/view/dashboard.js",
    include_bytes!("../assets/dashboard/js/view/dashboard.js"),
    JS,
  ),
  (
    "js/view/controls.js",
    include_bytes!("../assets/dashboard/js/view/controls.js"),
    JS,
  ),
  (
    "js/view/dom.js",
    include_bytes!("../assets/dashboard/js/view/dom.js"),
    JS,
  ),
  (
    "js/view/drawer.js",
    include_bytes!("../assets/dashboard/js/view/drawer.js"),
    JS,
  ),
  (
    "js/view/logs.js",
    include_bytes!("../assets/dashboard/js/view/logs.js"),
    JS,
  ),
  (
    "js/view/peer.js",
    include_bytes!("../assets/dashboard/js/view/peer.js"),
    JS,
  ),
  (
    "fonts/archivo.woff2",
    include_bytes!("../assets/dashboard/fonts/archivo.woff2"),
    FONT,
  ),
  (
    "fonts/jetbrains-mono.woff2",
    include_bytes!("../assets/dashboard/fonts/jetbrains-mono.woff2"),
    FONT,
  ),
];

/// An embedded asset and its HTTP media type.
pub(crate) struct Asset {
  pub bytes: &'static [u8],
  pub content_type: &'static str,
}

/// Find a public, versioned dashboard asset.
pub(crate) fn asset(path: &str) -> Option<Asset> {
  let file = path.strip_prefix(ASSET_PREFIX)?;
  ASSETS
    .iter()
    .find(|(name, _, _)| *name == file)
    .map(|&(_, bytes, content_type)| Asset {
      bytes,
      content_type,
    })
}

#[cfg(test)]
#[path = "tests/dashboard_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/dashboard_drawer_test.rs"]
mod drawer_tests;

#[cfg(test)]
#[path = "tests/dashboard_capture_test.rs"]
mod capture_tests;

#[cfg(test)]
#[path = "tests/dashboard_control_test.rs"]
mod control_tests;
