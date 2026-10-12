//! Rules that look past the file's text: the `../` import rule (10), gated on a
//! `@/` path alias in the nearest tsconfig up to the project root, and test
//! placement (20), which inspects the directory beside `__tests__/`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::file::{Pattern, TsFile, has_extension, is_test_path};

/// `jq -e '[(.compilerOptions.paths // {}) | keys[] | select(startswith("@/"))] | length > 0'`:
/// any error (bad JSON, a non-object where jq indexes) is "no alias".
fn declares_at_alias(config: &Path) -> bool {
  let Ok(text) = std::fs::read_to_string(config) else {
    return false;
  };
  let Ok(doc) = serde_json::from_str::<Value>(&text) else {
    return false;
  };
  doc
    .get("compilerOptions")
    .and_then(|options| options.get("paths"))
    .and_then(Value::as_object)
    .is_some_and(|paths| paths.keys().any(|key| key.starts_with("@/")))
}

fn is_file(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

/// `_ts_has_at_alias`: walk from the file's directory up to the project root (or `/`).
fn has_at_alias(f: &TsFile<'_>) -> bool {
  let root = f.at.project_root;
  let mut dir = f.file.absolute.parent();
  while let Some(here) = dir {
    for name in ["tsconfig.json", "tsconfig.base.json"] {
      let config = here.join(name);
      if is_file(&config) && declares_at_alias(&config) {
        return true;
      }
    }
    if here == root {
      return false;
    }
    dir = here.parent();
  }
  false
}

static PARENT_IMPORT: Pattern = Pattern::new(r#"from ["']\.\./"#);

/// 10-imports.
pub(crate) fn parent_import(f: &TsFile<'_>) -> Option<String> {
  if !f.lines.iter().any(|line| PARENT_IMPORT.is_match(line)) || !has_at_alias(f) {
    return None;
  }
  Some(format!(
    "Forbidden ../ import in {} — use @/ alias",
    f.path()
  ))
}

#[derive(PartialEq, Eq)]
enum Kind {
  File,
  Dir,
  Other,
}

fn kind(path: &Path) -> Kind {
  match std::fs::symlink_metadata(path) {
    Ok(meta) if meta.is_file() => Kind::File,
    Ok(meta) if meta.is_dir() => Kind::Dir,
    _ => Kind::Other,
  }
}

fn entries(dir: &Path) -> Vec<String> {
  std::fs::read_dir(dir).map_or_else(
    |_| Vec::new(),
    |list| {
      list
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
    },
  )
}

/// `find DIR -maxdepth 1 -type f` for a non-test `.ts`/`.tsx` source.
fn has_source(dir: &Path) -> bool {
  entries(dir).iter().any(|name| {
    has_extension(name, &["ts", "tsx"])
      && !name.contains(".test.")
      && !name.contains(".spec.")
      && !name.ends_with(".d.ts")
      && kind(&dir.join(name)) == Kind::File
  })
}

/// `find DIR -maxdepth 1 -type d ! -name __tests__ ! -name .`: DIR itself counts too.
fn has_other_dir(dir: &str, absolute: &Path) -> bool {
  let own = if dir == "/" {
    "/"
  } else {
    dir.trim_end_matches('/').rsplit('/').next().unwrap_or(dir)
  };
  if kind(absolute) == Kind::Dir && own != "__tests__" && own != "." {
    return true;
  }
  entries(absolute)
    .iter()
    .any(|name| name != "__tests__" && kind(&absolute.join(name)) == Kind::Dir)
}

/// Node's `dirname` of `<prefix>__tests__`: `.` for no directory part.
fn tests_parent(prefix: &str) -> String {
  let joined = format!("{prefix}__tests__");
  match Path::new(&joined).parent() {
    Some(parent) if parent.as_os_str().is_empty() => ".".to_owned(),
    Some(parent) => parent.to_string_lossy().into_owned(),
    None => "/".to_owned(),
  }
}

/// 20-tests: test files live flat in a `__tests__/` beside the code they test.
pub(crate) fn test_placement(f: &TsFile<'_>) -> Vec<String> {
  let path = f.path();
  if !is_test_path(path) || path.contains("/e2e/") {
    return Vec::new();
  }
  let Some(cut) = path
    .rfind("__tests__/")
    .filter(|_| path.contains("/__tests__/"))
  else {
    return vec![format!(
      "Test file outside __tests__/: {path} — move to sibling __tests__/ directory"
    )];
  };
  let mut errors = Vec::new();
  let (prefix, rest) = path.split_at(cut);
  let after = rest.strip_prefix("__tests__/").unwrap_or(rest);
  if let Some((sub, _)) = after.split_once('/')
    && !matches!(sub, "fixtures" | "helpers" | "utils")
  {
    errors.push(format!(
      "Test nested in __tests__/ subdirectory: {path} — keep __tests__/ flat (only fixtures/helpers/utils subdirs allowed; no mocks/ — tests must exercise real data)"
    ));
  }
  let parent = tests_parent(prefix);
  let absolute: PathBuf = f.at.cwd.join(&parent);
  if !has_source(&absolute) && !has_other_dir(&parent, &absolute) {
    errors.push(format!(
      "Test not co-located with source: {path} — __tests__/ must be at the same level as the code it tests"
    ));
  }
  errors
}

#[cfg(test)]
#[path = "tests/layout_rules_test.rs"]
mod tests;
