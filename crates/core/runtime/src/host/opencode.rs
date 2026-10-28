//! Where `OpenCode` loads plugins. `OpenCode` V2 loads every plugin under
//! `plugins/` of its global config directory, a directory through its
//! `server.js`, so `toolu plugins` copies each toolu plugin there as a directory
//! carrying the generated shim and the install marker of [`super::cursor`].

use std::path::PathBuf;

use super::roots::Roots;

/// `<config>/plugins`: the global plugins `OpenCode` loads.
#[must_use]
pub fn plugins_dir(roots: &Roots) -> PathBuf {
  roots.config_root().join("plugins")
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
