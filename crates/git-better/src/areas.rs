//! Where a change landed: each path maps to an area, the longest matching area
//! root plus its next segment (`crates/git-better`), else its first segment
//! (`docs`), else `.` for a file at the repository root.

use std::collections::BTreeMap;

use crate::gitio::FileChange;

/// The directories whose children are areas of their own.
pub(crate) const AREA_ROOTS: [&str; 7] = [
  "crates", "packages", "apps", "libs", "plugins", "services", "modules",
];

/// The changes in one area.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Area {
  /// The area, such as `crates/git-better`.
  pub(crate) name: String,
  /// Files changed.
  pub(crate) files: u64,
  /// Lines added, binary files not counted.
  pub(crate) added: u64,
  /// Lines deleted, binary files not counted.
  pub(crate) deleted: u64,
}

/// The area `path` belongs to under `roots`.
pub(crate) fn area_of(path: &str, roots: &[String]) -> String {
  let best = roots
    .iter()
    .filter(|root| {
      path
        .strip_prefix(root.as_str())
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|rest| rest.contains('/'))
    })
    .max_by_key(|root| root.len());
  if let Some(root) = best {
    let rest = path.get(root.len() + 1..).unwrap_or_default();
    let next = rest.split('/').next().unwrap_or_default();
    return format!("{root}/{next}");
  }
  match path.split_once('/') {
    Some((first, _)) => first.to_owned(),
    None => ".".to_owned(),
  }
}

/// `changes` grouped by area, in area order.
pub(crate) fn summarize(changes: &[FileChange], roots: &[String]) -> Vec<Area> {
  let mut areas: BTreeMap<String, Area> = BTreeMap::new();
  for change in changes {
    let name = area_of(&change.path, roots);
    let area = areas.entry(name.clone()).or_insert_with(|| Area {
      name,
      ..Area::default()
    });
    area.files += 1;
    area.added += change.added.unwrap_or_default();
    area.deleted += change.deleted.unwrap_or_default();
  }
  areas.into_values().collect()
}

#[cfg(test)]
#[path = "tests/areas_test.rs"]
mod tests;
