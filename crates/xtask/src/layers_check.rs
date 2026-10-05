//! `cargo xtask check-layers`: the layer graph of epic #402 and the capability
//! crates of rule 14, judged from `cargo metadata`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::data::{self, CapabilityCrates};
use crate::layers::{LayerTable, Role, forbidden_edge, may_build_binary};
use crate::metadata::{self, Metadata, Package};
use crate::options::Options;
use crate::workspace::relative;
use crate::{Verdict, output};

/// What a check found: crates and edges judged, and one line per violation.
pub(crate) struct Report {
  pub(crate) crates: usize,
  pub(crate) edges: usize,
  pub(crate) violations: Vec<String>,
}

struct Member<'a> {
  package: &'a Package,
  rel: PathBuf,
  role: Role,
}

/// Run the check on the workspace at `options.root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let text = std::fs::read_to_string(root.join(data::DATA_DIR).join("layers.json"))
    .map_err(|err| format!("cannot read {}/layers.json: {err}", data::DATA_DIR))?;
  let table =
    LayerTable::parse(&text).map_err(|err| format!("{}/layers.json: {err}", data::DATA_DIR))?;
  let rules = data::rules(root)?;
  let metadata = metadata::load(root)?;
  let report = check_layers(&metadata, &table, &rules.capability_crates);
  if report.violations.is_empty() {
    output::say(&format!(
      "check-layers: {} crates, {} edges, ok",
      report.crates, report.edges
    ));
    return Ok(Verdict::Clean);
  }
  for violation in &report.violations {
    output::error(&format!("check-layers: {violation}"));
  }
  Ok(Verdict::Findings)
}

/// Judge every member, edge and crate directory of `metadata`.
pub(crate) fn check_layers(
  metadata: &Metadata,
  table: &LayerTable,
  capabilities: &[CapabilityCrates],
) -> Report {
  let mut violations = Vec::new();
  let members: Vec<Member<'_>> = metadata
    .members()
    .filter_map(|package| {
      let rel = relative(&metadata.workspace_root, package.dir());
      match table.role(&rel) {
        Ok(role) => Some(Member { package, rel, role }),
        Err(reason) => {
          violations.push(format!("{} ({}): {reason}", rel.display(), package.name));
          None
        }
      }
    })
    .collect();
  let mut edges = 0;
  for member in &members {
    edges += check_edges(member, &members, &mut violations);
    violations.extend(capability_edges(member, capabilities));
    if member.package.has_target("bin") && !may_build_binary(member.role) {
      violations.push(format!(
        "{} ({}) builds a binary: only crates/{} builds one (crates/{} excepted)",
        member.rel.display(),
        member.package.name,
        table.binary,
        table.tooling
      ));
    }
  }
  violations.extend(unlisted_crates(metadata));
  Report {
    crates: members.len(),
    edges,
    violations,
  }
}

fn check_edges(member: &Member<'_>, members: &[Member<'_>], violations: &mut Vec<String>) -> usize {
  let mut edges = 0;
  for dep in member
    .package
    .dependencies
    .iter()
    .filter(|dep| dep.is_linked())
  {
    let Some(path) = &dep.path else { continue };
    let Some(target) = members
      .iter()
      .find(|other| other.package.dir() == path.as_path())
    else {
      continue;
    };
    edges += 1;
    if let Some(rule) = forbidden_edge(member.role, target.role) {
      violations.push(format!(
        "{} ({}, {}) depends on {} ({}, {}): {rule}",
        member.rel.display(),
        member.package.name,
        member.role,
        dep.name,
        target.rel.display(),
        target.role
      ));
    }
  }
  edges
}

/// Linked dependencies on a capability crate by anyone but its owner.
fn capability_edges(member: &Member<'_>, capabilities: &[CapabilityCrates]) -> Vec<String> {
  let mut violations = Vec::new();
  for dep in member
    .package
    .dependencies
    .iter()
    .filter(|dep| dep.is_linked())
  {
    for capability in capabilities {
      if capability.crates.contains(&dep.name) && capability.owner != member.package.name {
        violations.push(format!(
          "{} ({}) depends on {}: only {} may link a `{}` crate",
          member.rel.display(),
          member.package.name,
          dep.name,
          capability.owner,
          capability.id
        ));
      }
    }
  }
  violations
}

/// `crates/<name>/Cargo.toml` and `crates/core/<name>/Cargo.toml` files that are not members.
fn unlisted_crates(metadata: &Metadata) -> Vec<String> {
  let root = metadata.workspace_root.as_path();
  let listed: BTreeSet<PathBuf> = metadata
    .members()
    .map(|package| relative(root, package.dir()))
    .collect();
  let mut found = BTreeSet::new();
  for parent in ["crates", "crates/core"] {
    let entries = match std::fs::read_dir(root.join(parent)) {
      Ok(entries) => entries,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
      Err(err) => return vec![format!("cannot list {parent}: {err}")],
    };
    for entry in entries {
      match entry {
        Ok(entry) if entry.path().join("Cargo.toml").is_file() => {
          found.insert(Path::new(parent).join(entry.file_name()));
        }
        Ok(_) => {}
        Err(err) => return vec![format!("cannot list {parent}: {err}")],
      }
    }
  }
  found
    .into_iter()
    .filter(|dir| !listed.contains(dir.as_path()))
    .map(|dir| {
      format!(
        "{} has a Cargo.toml but is not a workspace member",
        dir.display()
      )
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/layers_check_test.rs"]
mod tests;
