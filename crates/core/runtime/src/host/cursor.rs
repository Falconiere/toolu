//! Where Cursor keeps plugins (#589). `cursor-agent plugin` only manages
//! marketplaces; a plugin dropped into `plugins/local/<name>` loads without
//! flags, and a marketplace plugin is cached at
//! `plugins/cache/<marketplace>/<plugin>/<rev>`. `toolu plugins` installs into
//! the first and `toolu doctor` reads both.

use std::path::PathBuf;

use super::roots::Roots;

/// The file `toolu plugins` writes into a local plugin directory it installed.
pub const MARKER: &str = ".toolu-install.json";

/// The file Cursor writes into a cached plugin revision once it is complete.
pub const CACHE_COMPLETE: &str = ".cache-complete";

/// `<config>/plugins/local`: the local plugins Cursor loads.
#[must_use]
pub fn local_dir(roots: &Roots) -> PathBuf {
  roots.config_root().join("plugins").join("local")
}

/// `<config>/plugins/cache/toolu`: the toolu marketplace's cached plugins.
#[must_use]
pub fn cache_dir(roots: &Roots) -> PathBuf {
  roots
    .config_root()
    .join("plugins")
    .join("cache")
    .join("toolu")
}

#[cfg(test)]
#[path = "tests/cursor_test.rs"]
mod tests;
