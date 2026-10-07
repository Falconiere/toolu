//! The registry directory listing (`registry-list.ts`). Order and admission match
//! `dispatch.sh` under `LC_ALL=C`: byte order, dotfiles hidden as a glob hides
//! them, regular files or symlinks to them only, and a module name without the
//! `<spec>__` namespace rejected rather than run.

pub(crate) mod bridge;
mod executable;
pub mod gate;
mod manifests;
pub(crate) mod phase;
pub mod prune;

use std::path::{Path, PathBuf};

use toolu_runtime::registry::{ModuleKind, NameParse, parse_name};

/// One module file of an event directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
  /// The base name, as stderr lines name the module.
  pub file: String,
  /// The full path.
  pub path: PathBuf,
  /// The owning plugin, `name@marketplace`.
  pub spec: String,
  /// The module name.
  pub name: String,
  /// What the file is.
  pub kind: ModuleKind,
}

/// An event directory's module files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
  /// The namespaced modules, in byte order of their names.
  pub entries: Vec<Entry>,
  /// Module files lacking the `<spec>__<name>` namespace, never run.
  pub rejected: Vec<String>,
}

/// Every `.js`, `.sh` and `.json` module file in `dir`, in byte order; an
/// absent or unreadable directory is empty. Names that are not UTF-8 are skipped.
pub fn list_dir(dir: &Path) -> Listing {
  let mut listing = Listing::default();
  for file in sorted_names(dir) {
    if file.starts_with('.') {
      continue;
    }
    let path = dir.join(&file);
    let parsed = parse_name(&file);
    if parsed == NameParse::NotModule || !is_file(&path) {
      continue;
    }
    match parsed {
      NameParse::Module(name) => listing.entries.push(Entry {
        file,
        path,
        spec: name.spec,
        name: name.name,
        kind: name.kind,
      }),
      NameParse::Unnamespaced | NameParse::NotModule => listing.rejected.push(file),
    }
  }
  listing
}

/// The UTF-8 names in `dir`, in byte order; none when it cannot be read.
pub(crate) fn sorted_names(dir: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(dir)
    .map(|read| {
      read
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect()
    })
    .unwrap_or_default();
  names.sort();
  names
}

/// A regular file, symlinks followed (`statSync(path).isFile()`).
fn is_file(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

#[cfg(test)]
#[path = "tests/registry_test.rs"]
mod tests;
