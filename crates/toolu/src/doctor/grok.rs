//! Active Grok plugins, including Claude-compatible installations.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use toolu_runtime::catalog::{BUILT_IN, plugin_names};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};

use super::inventory::{Inventory, plugin_at};

/// Read the plugins Grok would load in this workspace.
pub(super) fn collect(roots: &Roots, cwd: &Path) -> Inventory {
  let mut spec = Spec::new(["grok", "inspect", "--json"]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(roots.env().clone());
  spec.timeout = Duration::from_secs(10);
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.timed_out && !out.truncated)
    .and_then(|out| inventory(&serde_json::from_str(&out.stdout).ok()?))
    .unwrap_or_else(|| Inventory::warn("Grok plugin inventory is unavailable"))
}

fn inventory(report: &Value) -> Option<Inventory> {
  let names = plugin_names(BUILT_IN);
  let plugins = report
    .get("plugins")?
    .as_array()?
    .iter()
    .filter_map(|entry| {
      let name = entry.get("name")?.as_str()?;
      let path = PathBuf::from(entry.get("path")?.as_str()?);
      (entry.get("enabled")?.as_bool()? && names.iter().any(|item| item == name))
        .then(|| plugin_at(name, path))
    });
  Some(Inventory {
    plugins: plugins.collect(),
    warning: None,
  })
}

#[cfg(test)]
#[path = "tests/grok_test.rs"]
mod tests;
