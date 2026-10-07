//! Registry directories: bash modules are fine, the Bun bridge warns, a bad manifest fails.

use toolu_engine::registry::gate::plugin_presence;
use toolu_engine::registry::{Entry, list_dir};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::Installed;
use toolu_runtime::registry::manifest::{ModuleManifest, read_manifest};
use toolu_runtime::registry::{ModuleKind, RegistryEvent, event_dir};

use super::checks::{Check, Status};
use crate::tool_hook::RULES;

/// Walk `pre-tools.d` and `post-tools.d`. Orphan means the plugin is absent.
pub(super) fn check(roots: &Roots) -> Check {
  let mut found = Vec::new();
  for event in RegistryEvent::ALL {
    let listing = list_dir(&event_dir(roots, event));
    for file in listing.rejected {
      found.push((false, format!("{file} is unnamespaced")));
    }
    for entry in listing.entries {
      found.extend(entry_findings(roots, event, &entry));
    }
  }
  let failed = found.iter().any(|(fail, _)| *fail);
  let summary = if found.is_empty() {
    "no findings".to_owned()
  } else {
    found
      .iter()
      .map(|(_, text)| text.clone())
      .collect::<Vec<_>>()
      .join("; ")
  };
  let status = if failed {
    Status::Fail
  } else if found.is_empty() {
    Status::Ok
  } else {
    Status::Warn
  };
  Check::new("registry", status, summary, None, serde_json::json!({}))
}

fn entry_findings(roots: &Roots, event: RegistryEvent, entry: &Entry) -> Vec<(bool, String)> {
  match entry.kind {
    ModuleKind::Bash => Vec::new(),
    ModuleKind::Esm => vec![(
      false,
      format!("{} runs through the Bun bridge until #440", entry.file),
    )],
    ModuleKind::Manifest => manifest_findings(roots, event, entry),
  }
}

fn manifest_findings(roots: &Roots, event: RegistryEvent, entry: &Entry) -> Vec<(bool, String)> {
  let mut found = Vec::new();
  match read_manifest(&entry.path, event) {
    Err(reason) => found.push((true, reason)),
    Ok(manifest) if !compiled_in(&manifest, event) => {
      found.push((false, format!("{} rule missing", entry.file)));
    }
    Ok(_) => {}
  }
  if plugin_presence(&entry.spec, roots) == Installed::Absent {
    found.push((false, format!("{} is an orphaned manifest", entry.file)));
  }
  found
}

fn compiled_in(manifest: &ModuleManifest, event: RegistryEvent) -> bool {
  RULES.iter().any(|rule| {
    rule.spec() == manifest.spec && rule.name() == manifest.name && rule.event() == event
  })
}

#[cfg(test)]
#[path = "tests/registry_check_test.rs"]
mod tests;
