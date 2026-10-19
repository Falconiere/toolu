//! Every plugin ships a Muse manifest (#547) whose `name` and `description` equal the Claude
//! manifest's. Its version, release-please row, fields and agent commands are judged by
//! [`crate::packaging_muse`].

use std::path::Path;

use serde_json::Value;

use crate::packaging_assets::read_json;

/// Fail unless `plugins/<name>/.muse-plugin/plugin.json` exists and matches `claude`.
pub(crate) fn check(
  root: &Path,
  plugin_root: &str,
  name: &str,
  claude: &Value,
) -> Result<(), String> {
  let path = format!("{plugin_root}/.muse-plugin/plugin.json");
  if !root.join(&path).is_file() {
    return Err(format!("{name} is missing its Muse manifest"));
  }
  let muse = read_json(root, &path)?;
  for key in ["name", "description"] {
    if muse.get(key) != claude.get(key) {
      return Err(format!("{name} Muse manifest {key} differs from Claude"));
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/packaging_muse_identity_test.rs"]
mod tests;
