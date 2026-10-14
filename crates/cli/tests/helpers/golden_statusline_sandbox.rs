//! A disposable per-case sandbox and the `$path` / `$template` expansion of the case file.

use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde_json::{Map, Value};
use tempfile::TempDir;

use crate::golden_statusline_value::TestResult;
use crate::golden_statusline_value::bad;

/// A temp project, HOME and Codex home owned by one value; dropping it removes the tree.
pub(crate) struct Sandbox {
  _dir: TempDir,
  pub(crate) root: PathBuf,
  pub(crate) project: PathBuf,
  pub(crate) home: PathBuf,
}

/// `base/rel`, refusing anything that is not a plain descent.
fn inside(base: &Path, rel: &str) -> TestResult<PathBuf> {
  if Path::new(rel)
    .components()
    .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
  {
    return Err(bad(format!("fixture path escapes the sandbox: {rel}")));
  }
  Ok(base.join(rel))
}

impl Sandbox {
  /// Create the sandbox; macOS temp dirs are symlinks, so the root is canonical.
  pub(crate) fn new() -> TestResult<Self> {
    let dir = tempfile::tempdir()?;
    let root = std::fs::canonicalize(dir.path())?;
    let project = root.join("project");
    let home = root.join("home");
    std::fs::create_dir_all(&project)?;
    std::fs::create_dir_all(home.join(".codex"))?;
    Ok(Self {
      _dir: dir,
      root,
      project,
      home,
    })
  }

  /// `<project>/<rel>`, the way the TypeScript sandbox's `path` did.
  pub(crate) fn path(&self, rel: &str) -> TestResult<PathBuf> {
    inside(&self.project, rel)
  }

  fn tokens(&self) -> [(&'static str, PathBuf); 4] {
    [
      ("$PROJECT", self.project.clone()),
      ("$ROOT", self.root.clone()),
      ("$HOME", self.home.clone()),
      ("$HOST_STATE", self.project.join(".claude")),
    ]
  }

  /// Resolve a path field that starts with a sandbox token, or is relative to the project.
  pub(crate) fn resolve(&self, path: &str) -> TestResult<PathBuf> {
    for (token, base) in self.tokens() {
      if path == token {
        return Ok(base);
      }
      if let Some(rest) = path.strip_prefix(&format!("{token}/")) {
        return inside(&base, rest);
      }
    }
    if path.starts_with('$') {
      return Err(bad(format!("unknown path token: {path}")));
    }
    self.path(path)
  }

  /// Resolve a mutation path, refusing a symlink on the way (the last one when not allowed).
  pub(crate) fn mutation_path(&self, path: &str, final_symlink: bool) -> TestResult<PathBuf> {
    let abs = self.resolve(path)?;
    let back = abs
      .strip_prefix(&self.root)
      .map_err(|_outside| bad(format!("fixture path escapes the sandbox: {path}")))?;
    let count = back.components().count();
    let mut current = self.root.clone();
    for (index, part) in back.components().enumerate() {
      current.push(part);
      let link = std::fs::symlink_metadata(&current).is_ok_and(|meta| meta.is_symlink());
      if link && !(final_symlink && index + 1 == count) {
        return Err(bad(format!("fixture mutation follows a symlink: {path}")));
      }
    }
    Ok(abs)
  }

  fn expand(&self, template: &str) -> TestResult<String> {
    let token = Regex::new(r"\$[A-Z][A-Z_]*")?;
    let mut out = String::new();
    let mut last = 0;
    for found in token.find_iter(template) {
      let after = template.get(found.end()..).unwrap_or("");
      let glued = after
        .chars()
        .next()
        .is_some_and(|next| next.is_alphanumeric() || next == '_');
      out.push_str(template.get(last..found.start()).unwrap_or(""));
      last = found.end();
      if glued {
        out.push_str(found.as_str());
        continue;
      }
      let base = self
        .tokens()
        .into_iter()
        .find(|(name, _)| *name == found.as_str())
        .map(|(_, base)| base)
        .ok_or_else(|| bad(format!("unknown path token: {}", found.as_str())))?;
      out.push_str(&base.to_string_lossy());
    }
    out.push_str(template.get(last..).unwrap_or(""));
    Ok(out)
  }

  /// Expand the explicitly tagged dynamic values, leaving ordinary text unchanged.
  pub(crate) fn materialize(&self, value: &Value) -> TestResult<Value> {
    match value {
      Value::Array(items) => Ok(Value::Array(
        items
          .iter()
          .map(|item| self.materialize(item))
          .collect::<TestResult<Vec<_>>>()?,
      )),
      Value::Object(map) => self.materialize_object(map),
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(value.clone()),
    }
  }

  fn materialize_object(&self, map: &Map<String, Value>) -> TestResult<Value> {
    if map.len() == 1 {
      if let Some(Value::String(path)) = map.get("$path") {
        return Ok(Value::String(
          self.resolve(path)?.to_string_lossy().into_owned(),
        ));
      }
      if let Some(Value::String(template)) = map.get("$template") {
        return Ok(Value::String(self.expand(template)?));
      }
    }
    let mut out = Map::new();
    for (key, item) in map {
      out.insert(key.clone(), self.materialize(item)?);
    }
    Ok(Value::Object(out))
  }

  /// A tagged fixture value that must be a string.
  pub(crate) fn string(&self, value: &Value) -> TestResult<String> {
    match self.materialize(value)? {
      Value::String(text) => Ok(text),
      other @ (Value::Null
      | Value::Bool(_)
      | Value::Number(_)
      | Value::Array(_)
      | Value::Object(_)) => Err(bad(format!("expected a string fixture value, got {other}"))),
    }
  }
}
