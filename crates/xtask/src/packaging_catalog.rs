//! Marketplace, cargo, and symlink checks for `cargo xtask packaging`.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

const CARGO_RELEASE: [(&str, &str); 2] = [
  ("Cargo.toml", "$.workspace.package.version"),
  (
    "Cargo.lock",
    "$.package[?(!@.source && @.name.value != 'tree-sitter-bash')].version",
  ),
];

pub(crate) fn check_catalogs(
  name: &str,
  claude_manifest: &Value,
  claude_catalog: &Value,
  codex_catalog: &Value,
) -> Result<(), String> {
  let claude_entries = named(claude_catalog, name);
  let want = format!("./plugins/{name}");
  let source = claude_entries
    .first()
    .and_then(|entry| entry.get("source"))
    .and_then(Value::as_str);
  if claude_entries.is_empty() || source != Some(want.as_str()) {
    return Err(format!(
      "{name} is missing or has the wrong Claude marketplace source"
    ));
  }
  if claude_entries.len() != 1 {
    return Err(format!(
      "{name} must appear exactly once in the Claude marketplace"
    ));
  }
  let description = claude_entries
    .first()
    .and_then(|entry| entry.get("description"))
    .and_then(Value::as_str);
  if description != claude_manifest.get("description").and_then(Value::as_str) {
    return Err(format!(
      "{name} marketplace description differs from its manifest"
    ));
  }
  let codex_entries = named(codex_catalog, name);
  match codex_entries.as_slice() {
    [entry] => codex_rules(name, entry),
    [] => Err(format!("{name} is missing from the Codex marketplace")),
    _ => Err(format!(
      "{name} must appear exactly once in the Codex marketplace"
    )),
  }
}

fn codex_rules(name: &str, entry: &Value) -> Result<(), String> {
  let path = format!("./plugins/{name}");
  let rules: [(&[&str], &str, &str); 5] = [
    (
      &["source", "source"],
      "local",
      "Codex marketplace source must be local",
    ),
    (
      &["source", "path"],
      path.as_str(),
      "Codex marketplace source path is wrong",
    ),
    (
      &["policy", "installation"],
      "AVAILABLE",
      "Codex marketplace installation policy is wrong",
    ),
    (
      &["policy", "authentication"],
      "ON_INSTALL",
      "Codex marketplace authentication policy is wrong",
    ),
    (
      &["category"],
      "Productivity",
      "Codex marketplace category is wrong",
    ),
  ];
  for (keys, want, message) in rules {
    if dig(entry, keys).and_then(Value::as_str) != Some(want) {
      return Err(format!("{name} {message}"));
    }
  }
  Ok(())
}

pub(crate) fn check_cargo(root: &Path, version: &str, release: &Value) -> Result<(), String> {
  for (path, jsonpath) in CARGO_RELEASE {
    if !release_tracks(release, path, "toml", jsonpath) {
      return Err(format!("release-please is missing {path} ({jsonpath})"));
    }
  }
  let cargo = read_toml(root, "Cargo.toml")?;
  let got = cargo
    .get("workspace")
    .and_then(|doc| doc.get("package"))
    .and_then(|doc| doc.get("version"))
    .and_then(toml::Value::as_str);
  if got != Some(version) {
    return Err("Cargo.toml [workspace.package] version differs from package.json".to_owned());
  }
  let lock = read_toml(root, "Cargo.lock")?;
  let packages = lock
    .get("package")
    .and_then(toml::Value::as_array)
    .ok_or_else(|| "Cargo.lock lists no workspace crate".to_owned())?;
  let mut local = 0;
  for pkg in packages {
    if pkg.get("source").is_some() {
      continue;
    }
    let name = pkg.get("name").and_then(toml::Value::as_str).unwrap_or("");
    if name == "tree-sitter-bash" {
      continue;
    }
    local += 1;
    if pkg.get("version").and_then(toml::Value::as_str) != Some(version) {
      return Err(format!(
        "Cargo.lock {name} version differs from package.json"
      ));
    }
  }
  if local == 0 {
    return Err("Cargo.lock lists no workspace crate".to_owned());
  }
  Ok(())
}

pub(crate) fn check_symlinks(root: &Path, plugin_root: &str, name: &str) -> Result<(), String> {
  let plugin = root.join(plugin_root);
  let root_real =
    std::fs::canonicalize(&plugin).map_err(|err| format!("cannot resolve {plugin_root}: {err}"))?;
  let mut stack = vec![plugin];
  while let Some(dir) = stack.pop() {
    for entry in
      std::fs::read_dir(&dir).map_err(|err| format!("cannot list {}: {err}", dir.display()))?
    {
      let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
      let path = entry.path();
      let kind = entry
        .file_type()
        .map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
      if kind.is_symlink() {
        one_link(root, &root_real, name, &path)?;
      } else if kind.is_dir() {
        stack.push(path);
      }
    }
  }
  Ok(())
}

fn one_link(repo: &Path, root_real: &Path, name: &str, path: &Path) -> Result<(), String> {
  let target = std::fs::read_link(path)
    .map_err(|err| format!("cannot read link {}: {err}", path.display()))?;
  let resolved = resolve_link(path, &target);
  let prefix = format!("{}/", root_real.display());
  if resolved.display().to_string().starts_with(&prefix) {
    return Ok(());
  }
  let rel = path.strip_prefix(repo).unwrap_or(path);
  Err(format!(
    "{name} symlink escapes plugin root: {} -> {}",
    rel.display(),
    target.display()
  ))
}

fn resolve_link(path: &Path, target: &Path) -> PathBuf {
  if target.is_absolute() {
    return normalize(target);
  }
  let parent = path.parent().unwrap_or(path);
  let base = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
  normalize(&base.join(target))
}

fn normalize(path: &Path) -> PathBuf {
  let mut out = PathBuf::new();
  for part in path.components() {
    match part {
      Component::ParentDir => {
        out.pop();
      }
      Component::CurDir => {}
      Component::Prefix(prefix) => out.push(prefix.as_os_str()),
      Component::RootDir => out.push("/"),
      Component::Normal(part) => out.push(part),
    }
  }
  out
}

pub(crate) fn release_tracks(release: &Value, path: &str, kind: &str, jsonpath: &str) -> bool {
  dig(release, &["packages", ".", "extra-files"])
    .and_then(Value::as_array)
    .is_some_and(|entries| {
      entries.iter().any(|entry| {
        entry.get("type").and_then(Value::as_str) == Some(kind)
          && entry.get("path").and_then(Value::as_str) == Some(path)
          && entry.get("jsonpath").and_then(Value::as_str) == Some(jsonpath)
      })
    })
}

fn named<'a>(catalog: &'a Value, name: &str) -> Vec<&'a Value> {
  catalog
    .get("plugins")
    .and_then(Value::as_array)
    .map(|entries| {
      entries
        .iter()
        .filter(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
        .collect()
    })
    .unwrap_or_default()
}

fn dig<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
  let mut cur = value;
  for key in keys {
    cur = cur.as_object()?.get(*key)?;
  }
  Some(cur)
}

pub(crate) fn array_len(doc: &Value, key: &str) -> usize {
  doc
    .get(key)
    .and_then(Value::as_array)
    .map_or(0, std::vec::Vec::len)
}

fn read_toml(root: &Path, rel: &str) -> Result<toml::Value, String> {
  let path = root.join(rel);
  if !path.is_file() {
    return Err(format!("missing {rel}"));
  }
  let text = std::fs::read_to_string(&path).map_err(|err| format!("invalid TOML: {rel}: {err}"))?;
  toml::from_str(&text).map_err(|err| format!("invalid TOML: {rel}: {err}"))
}

#[cfg(test)]
#[path = "tests/packaging_catalog_test.rs"]
mod tests;
