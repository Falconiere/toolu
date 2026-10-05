//! The version-skew rule of #411: a different `hookProtocol` fails closed, while
//! any semver difference with the same protocol only advises.

use std::path::Path;

use crate::manifest::Manifest;
use crate::version::{Comparison, compare};

/// The advice that ends every line asking for newer plugins.
const UPDATE_PLUGINS: &str = "update the plugins from your host's marketplace";

/// The running binary.
#[derive(Debug, Clone, Copy)]
pub struct Binary<'a> {
  /// Its semver.
  pub version: &'a str,
  /// Its compiled `HOOK_PROTOCOL`.
  pub protocol: u32,
}

/// The plugin whose hook is running.
#[derive(Debug, Clone, Copy)]
pub struct Caller<'a> {
  /// Plugin name, as in `hooks.json`.
  pub plugin: &'a str,
  /// Its root, where `plugin.json` was read.
  pub root: &'a Path,
}

/// What the rule decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skew {
  /// Same protocol and version: run, say nothing.
  Same,
  /// Same protocol, different version: run, and advise at `SessionStart`.
  Advise(String),
  /// Different or unreadable protocol: enforcing events block, context events
  /// print it; the hook does not run.
  Mismatch(String),
}

/// Judge `manifest` (as read from `caller.root`) against `binary`; `upgrade` is
/// the command that upgrades this binary.
pub fn assess(
  caller: &Caller<'_>,
  binary: &Binary<'_>,
  manifest: &Result<Manifest, String>,
  upgrade: &str,
) -> Skew {
  let (plugin, b, n) = (caller.plugin, binary.version, binary.protocol);
  let manifest = match manifest {
    Ok(manifest) => manifest,
    Err(reason) => {
      let root = caller.root.display();
      return Skew::Mismatch(format!(
        "{plugin} plugin: cannot read hookProtocol from {root}: {reason}; {UPDATE_PLUGINS}"
      ));
    }
  };
  let (p, wanted) = (manifest.version.as_str(), manifest.hook_protocol);
  if wanted > n {
    return Skew::Mismatch(format!(
      "{plugin} plugin: hook protocol {wanted} needs a newer toolu - toolu {b} speaks \
       protocol {n}; upgrade it: {upgrade}"
    ));
  }
  if wanted < n {
    return Skew::Mismatch(format!(
      "{plugin} plugin: hook protocol {wanted} is older than toolu {b} - protocol {n}; \
       {UPDATE_PLUGINS}"
    ));
  }
  let advice = match compare(b, p) {
    Comparison::Same => return Skew::Same,
    Comparison::Older => {
      return Skew::Advise(format!(
        "toolu {b} is older than the {plugin} plugin {p}; upgrade it: {upgrade}"
      ));
    }
    Comparison::Newer => "is newer than",
    Comparison::Differs => "does not match",
    Comparison::Incomparable => "cannot be compared with",
  };
  Skew::Advise(format!(
    "toolu {b} {advice} the {plugin} plugin {p}; {UPDATE_PLUGINS}"
  ))
}

#[cfg(test)]
#[path = "tests/skew_test.rs"]
mod tests;
