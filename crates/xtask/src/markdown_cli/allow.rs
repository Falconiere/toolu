//! The allowlists: `tooling/conventions/markdown-cli.json` for every scanned
//! file and `tooling/conventions/markdown-cli/<plugin>.json` for the files
//! under `plugins/<plugin>/` (outside the plugin, so it is not shipped). Each
//! names `external` commands that are not `toolu`, and `allow` entries that
//! excuse one finding (file and subject) with a reason. An entry that excuses
//! nothing is stale and fails, so an allowance cannot outlive its finding.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use super::Finding;
use super::scan::entries;

/// The repository-wide allowlist.
pub(crate) const REPOSITORY: &str = "tooling/conventions/markdown-cli.json";

/// The directory of per-plugin allowlists, `<plugin>.json` each.
const PLUGINS: &str = "tooling/conventions/markdown-cli";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
  #[serde(default)]
  external: Vec<String>,
  #[serde(default)]
  allow: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
  file: String,
  subject: String,
  reason: String,
}

/// One loaded allowlist.
#[derive(Debug)]
pub(crate) struct Allowlist {
  /// Its repository-relative path.
  source: String,
  /// The prefix its files share: empty for the repository-wide list.
  scope: String,
  data: Data,
}

/// Load the repository-wide list and every plugin's; absent files are empty.
pub(crate) fn load(root: &Path) -> Result<Vec<Allowlist>, String> {
  let mut lists = Vec::new();
  lists.extend(read(root, REPOSITORY.to_owned(), String::new())?);
  for name in entries(&root.join(PLUGINS))? {
    let source = format!("{PLUGINS}/{name}");
    let plugin = name
      .strip_suffix(".json")
      .filter(|plugin| root.join("plugins").join(plugin).is_dir());
    let Some(plugin) = plugin else {
      return Err(format!(
        "{source}: not `<plugin>.json` for a directory under plugins/"
      ));
    };
    lists.extend(read(root, source, format!("plugins/{plugin}/"))?);
  }
  Ok(lists)
}

fn read(root: &Path, source: String, scope: String) -> Result<Option<Allowlist>, String> {
  let text = match std::fs::read_to_string(root.join(&source)) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(format!("cannot read {source}: {err}")),
  };
  let data: Data = serde_json::from_str(&text).map_err(|err| format!("{source}: {err}"))?;
  for entry in &data.allow {
    if entry.reason.trim().is_empty() {
      return Err(format!(
        "{source}: the allowance for `{}` in {} needs a reason",
        entry.subject, entry.file
      ));
    }
    if !entry.file.starts_with(&scope) {
      return Err(format!(
        "{source}: {} is outside this allowlist's plugin ({scope})",
        entry.file
      ));
    }
  }
  Ok(Some(Allowlist {
    source,
    scope,
    data,
  }))
}

/// The external commands `file` may run: the repository's and its plugin's.
pub(crate) fn external(lists: &[Allowlist], file: &str) -> BTreeSet<String> {
  lists
    .iter()
    .filter(|list| file.starts_with(&list.scope))
    .flat_map(|list| list.data.external.iter().cloned())
    .collect()
}

/// The findings no entry excuses, then one line per stale entry.
pub(crate) fn apply(lists: &[Allowlist], findings: Vec<Finding>) -> Vec<String> {
  let mut used = BTreeSet::new();
  let mut out = Vec::new();
  for finding in findings {
    let excuse = lists.iter().enumerate().find_map(|(at, list)| {
      list
        .data
        .allow
        .iter()
        .position(|entry| entry.file == finding.file && entry.subject == finding.subject)
        .map(|index| (at, index))
    });
    match excuse {
      Some(key) => {
        used.insert(key);
      }
      None => out.push(finding.to_string()),
    }
  }
  for (at, list) in lists.iter().enumerate() {
    for (index, entry) in list.data.allow.iter().enumerate() {
      if !used.contains(&(at, index)) {
        out.push(format!(
          "{}: stale allowance `{}` in {}: it no longer fails, remove it",
          list.source, entry.subject, entry.file
        ));
      }
    }
  }
  out
}

#[cfg(test)]
#[path = "tests/allow_test.rs"]
mod tests;
