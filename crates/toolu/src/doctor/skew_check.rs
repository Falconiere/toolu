//! Version and hookProtocol skew against each installed plugin (#411). A
//! plugin the marketplace retired is set aside and named with the command that
//! removes it, because no update can bring it to this binary's protocol (#615).

use std::path::PathBuf;

use serde_json::json;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::host::Host;
use toolu_runtime::catalog;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest::{self, Manifest};
use toolu_runtime::skew::{self, Skew};
use toolu_runtime::version::{Comparison, compare};

use super::checks::{Check, Status};
use super::inventory::Inventory;

/// What the installed plugins came to, in inventory order.
#[derive(Default)]
struct Findings<'a> {
  retired: Vec<&'a str>,
  advises: Vec<String>,
  mismatches: Vec<String>,
}

/// Mismatch fails; a retired plugin or advice warns.
pub(super) fn check(inventory: &Inventory, roots: &Roots) -> Check {
  let found = assess(inventory);
  let hint = removal(roots, &found.retired);
  let details = json!({ "retired": found.retired });
  let note = retired_note(&found.retired);
  if !found.mismatches.is_empty() {
    let summary = found.mismatches.into_iter().chain(note).collect::<Vec<_>>();
    return Check::new("skew", Status::Fail, summary.join("; "), hint, details);
  }
  let warnings = note.into_iter().chain(found.advises).collect::<Vec<_>>();
  if !warnings.is_empty() {
    return Check::new("skew", Status::Warn, warnings.join("; "), hint, details);
  }
  let summary = if inventory.roots().next().is_none() {
    "no plugins to compare"
  } else {
    "plugin versions match"
  };
  Check::new("skew", Status::Ok, summary, None, details)
}

fn assess(inventory: &Inventory) -> Findings<'_> {
  let upgrade =
    upgrade_command(&std::env::current_exe().unwrap_or_else(|_| PathBuf::from("toolu")));
  let binary = skew::Binary {
    version: env!("CARGO_PKG_VERSION"),
    protocol: HOOK_PROTOCOL,
  };
  let shipped = catalog::plugin_names(catalog::BUILT_IN);
  let mut found = Findings::default();
  for (name, root) in inventory.roots() {
    let manifest = manifest::read(root);
    if retired(name, &manifest, binary.version, &shipped) {
      if !found.retired.contains(&name) {
        found.retired.push(name);
      }
      continue;
    }
    let caller = skew::Caller { plugin: name, root };
    match skew::assess(&caller, &binary, &manifest, upgrade) {
      Skew::Same => {}
      Skew::Advise(text) => found.advises.push(text),
      Skew::Mismatch(text) => found.mismatches.push(text),
    }
  }
  found
}

/// Whether installed plugin `name` is one the marketplace retired: it is not in
/// the catalog built into this binary, and it is not newer than this binary.
/// Every plugin since the Rust rewrite declares a `hookProtocol`, so one without
/// it is old; a plugin released after this build keeps the upgrade advice. An
/// empty catalog retires nothing.
fn retired(
  name: &str,
  manifest: &Result<Manifest, String>,
  binary: &str,
  shipped: &[String],
) -> bool {
  if shipped.is_empty() || shipped.iter().any(|known| known == name) {
    return false;
  }
  match manifest {
    Ok(manifest) => compare(binary, &manifest.version) != Comparison::Older,
    Err(_unreadable) => true,
  }
}

/// `retired plugin a is …` or `retired plugins a, b are …`; `None` for none.
fn retired_note(retired: &[&str]) -> Option<String> {
  match retired {
    [] => None,
    [one] => Some(format!(
      "retired plugin {one} is no longer in the toolu marketplace"
    )),
    many => Some(format!(
      "retired plugins {} are no longer in the toolu marketplace",
      many.join(", ")
    )),
  }
}

/// The command that removes `retired` on the host doctor inspected. Muse
/// removes by bare name from any source, so `toolu plugins` refuses a name
/// outside the catalog there and Muse's own command is named instead.
pub(super) fn removal(roots: &Roots, retired: &[&str]) -> Option<String> {
  if retired.is_empty() {
    return None;
  }
  let host = roots.host();
  match host {
    Host::Muse => Some(
      retired
        .iter()
        .filter_map(|name| roots.plugin_uninstall_command(name).ok().flatten())
        .collect::<Vec<_>>()
        .join("; "),
    ),
    Host::Hermes => None,
    Host::Claude | Host::Codex | Host::Cursor | Host::Opencode => Some(format!(
      "toolu plugins remove {} --host {} --yes",
      retired.join(" "),
      host.name()
    )),
  }
}

#[cfg(test)]
#[path = "tests/skew_check_test.rs"]
mod tests;
