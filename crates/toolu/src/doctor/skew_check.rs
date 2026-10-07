//! Version and hookProtocol skew against each installed plugin (#411).

use std::path::PathBuf;

use serde_json::json;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::install::upgrade_command;
use toolu_runtime::manifest;
use toolu_runtime::skew::{self, Skew};

use super::checks::{Check, Status};
use super::inventory::Inventory;

/// `OpenCode` has no native skew. Elsewhere, mismatch fails and advice warns.
pub(super) fn check(roots: &Roots, inventory: &Inventory) -> Check {
  if roots.host() == Host::Opencode {
    return Check::new("skew", Status::Ok, "not applicable", None, json!({}));
  }
  let upgrade =
    upgrade_command(&std::env::current_exe().unwrap_or_else(|_| PathBuf::from("toolu")));
  let binary = skew::Binary {
    version: env!("CARGO_PKG_VERSION"),
    protocol: HOOK_PROTOCOL,
  };
  let mut advises = Vec::new();
  let mut mismatches = Vec::new();
  for (name, root) in inventory.roots() {
    let manifest = manifest::read(root);
    let caller = skew::Caller { plugin: name, root };
    match skew::assess(&caller, &binary, &manifest, upgrade) {
      Skew::Same => {}
      Skew::Advise(text) => advises.push(text),
      Skew::Mismatch(text) => mismatches.push(text),
    }
  }
  if !mismatches.is_empty() {
    return Check::new("skew", Status::Fail, mismatches.join("; "), None, json!({}));
  }
  if !advises.is_empty() {
    return Check::new("skew", Status::Warn, advises.join("; "), None, json!({}));
  }
  let summary = if inventory.roots().next().is_none() {
    "no plugins to compare"
  } else {
    "plugin versions match"
  };
  Check::new("skew", Status::Ok, summary, None, json!({}))
}

#[cfg(test)]
#[path = "tests/skew_check_test.rs"]
mod tests;
