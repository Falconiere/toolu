//! Native `SessionStart`: retain the old stable path as a one-line shell shim.

use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::publish::{PublishOptions, publish};
use toolu_runtime::startup::report::HelperStatus;

/// Link the compatibility shim into the active host's config root.
pub fn session_start(env: &Env, plugin_root: &Path) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let source = plugin_root.join("scripts/write-state.sh");
  let published = publish(&PublishOptions {
    plugin: "toolu-review",
    source: &source,
    dir: "toolu-review",
    name: "write-state.sh",
    what: "helper",
    roots: &roots,
  });
  if let Some(error) = published.report_error {
    return Outcome::failed(Exit::Failure, error);
  }
  let warning = published.warning.or_else(|| match published.result.status {
    HelperStatus::LinkFailed => published
      .result
      .path
      .map(|path| format!("toolu-review: cannot publish {}", path.display())),
    HelperStatus::SourceMissing => Some("toolu-review: compatibility helper missing".to_owned()),
    HelperStatus::Published | HelperStatus::KeptUserFile | HelperStatus::Unwritable => None,
  });
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: warning,
  }
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
