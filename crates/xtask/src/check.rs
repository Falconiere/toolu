//! `cargo xtask check-layers`: the layer graph of epic #402, judged from `cargo metadata`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::layers::{LayerTable, Role, forbidden_edge, may_build_binary};
use crate::metadata::{Metadata, Package};

/// What a check found: crates and edges judged, and one line per violation.
pub struct Report {
  pub crates: usize,
  pub edges: usize,
  pub violations: Vec<String>,
}

struct Member<'a> {
  package: &'a Package,
  rel: PathBuf,
  role: Role,
}

/// Judge every member, edge and crate directory of `metadata` against `table`.
pub fn check_layers(metadata: &Metadata, table: &LayerTable) -> Report {
  let mut violations = Vec::new();
  let members: Vec<Member<'_>> = metadata
    .packages
    .iter()
    .filter(|package| metadata.workspace_members.contains(&package.id))
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
    if member.package.builds_binary() && !may_build_binary(member.role) {
      violations.push(format!(
        "{} ({}) builds a binary: only crates/cli builds one (crates/xtask excepted)",
        member.rel.display(),
        member.package.name
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

/// `crates/<name>/Cargo.toml` and `crates/core/<name>/Cargo.toml` files that are not members.
fn unlisted_crates(metadata: &Metadata) -> Vec<String> {
  let root = metadata.workspace_root.as_path();
  let listed: BTreeSet<PathBuf> = metadata
    .packages
    .iter()
    .filter(|package| metadata.workspace_members.contains(&package.id))
    .map(|package| relative(root, package.dir()))
    .collect();
  let mut found = BTreeSet::new();
  for parent in ["crates", "crates/core"] {
    let Ok(entries) = std::fs::read_dir(root.join(parent)) else {
      continue;
    };
    for entry in entries.flatten() {
      if entry.path().join("Cargo.toml").is_file() {
        found.insert(Path::new(parent).join(entry.file_name()));
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

fn relative(root: &Path, dir: &Path) -> PathBuf {
  dir
    .strip_prefix(root)
    .map(Path::to_path_buf)
    .unwrap_or_else(|_| dir.to_path_buf())
}
