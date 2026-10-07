//! Manifests, `<spec>__<name>.json`: each enables the compiled-in rule of that
//! spec and name for its event. All of a directory's manifests are read when the
//! registry phase starts, so a usable one can shadow its spec's older modules.

use std::collections::{BTreeMap, BTreeSet};

use toolu_runtime::registry::manifest::{ModuleManifest, read_manifest};
use toolu_runtime::registry::rule::Rule;
use toolu_runtime::registry::{ModuleKind, RegistryEvent};

use super::Entry;

/// One manifest as the walk sees it.
pub(crate) enum Read<'r> {
  /// Unreadable or invalid: the reason, without the path `read_manifest` puts first.
  Problem(String),
  /// Valid, with the compiled-in rule it names, if this binary has it.
  Valid(ModuleManifest, Option<&'r dyn Rule>),
}

/// Every manifest of a listing, by file name.
pub(crate) struct Manifests<'r> {
  read: BTreeMap<String, Read<'r>>,
}

impl<'r> Manifests<'r> {
  /// Reads every manifest among `entries` for `event`, looking rules up in `rules`.
  pub(crate) fn read(
    entries: &[Entry],
    event: RegistryEvent,
    rules: &[&'r dyn Rule],
  ) -> Manifests<'r> {
    let read = entries
      .iter()
      .filter(|entry| entry.kind == ModuleKind::Manifest)
      .map(|entry| (entry.file.clone(), read_one(entry, event, rules)))
      .collect();
    Manifests { read }
  }

  /// The manifest of `file`.
  pub(crate) fn get(&self, file: &str) -> Option<&Read<'r>> {
    self.read.get(file)
  }

  /// The specs with a usable manifest: valid, and its rule compiled in.
  pub(crate) fn usable_specs(&self) -> BTreeSet<String> {
    self
      .read
      .values()
      .filter_map(|read| match read {
        Read::Valid(manifest, Some(_)) => Some(manifest.spec.clone()),
        Read::Valid(_, None) | Read::Problem(_) => None,
      })
      .collect()
  }
}

/// One manifest and the rule it names.
fn read_one<'r>(entry: &Entry, event: RegistryEvent, rules: &[&'r dyn Rule]) -> Read<'r> {
  match read_manifest(&entry.path, event) {
    Ok(manifest) => {
      let named = |rule: &&&'r dyn Rule| {
        rule.spec() == manifest.spec && rule.name() == manifest.name && rule.event() == event
      };
      let rule = rules.iter().find(named).copied();
      Read::Valid(manifest, rule)
    }
    Err(reason) => {
      let prefix = format!("{}: ", entry.path.display());
      Read::Problem(reason.strip_prefix(&prefix).unwrap_or(&reason).to_owned())
    }
  }
}

#[cfg(test)]
#[path = "tests/manifests_test.rs"]
mod tests;
