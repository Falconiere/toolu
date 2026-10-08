//! Stderr lines the lifecycle hooks share: host detection, then config warnings.

use toolu_runtime::config::load::{LoadedConfig, WARN_PREFIX};

/// Host detection, prefixed config warnings, then `extra`. Empty is silence.
pub(crate) fn diagnostics(
  host_warning: Option<String>,
  config: &LoadedConfig,
  extra: Vec<String>,
) -> Option<String> {
  let mut lines = Vec::new();
  if let Some(warning) = host_warning {
    lines.push(warning);
  }
  for message in config.take_warnings() {
    lines.push(format!("{WARN_PREFIX}{message}"));
  }
  lines.extend(extra);
  (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
#[path = "tests/diagnostics_test.rs"]
mod tests;
