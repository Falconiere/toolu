//! Sandbox paths and setup actions: the `$PROJECT`, `$ROOT`, `$HOME` and `$HOST_STATE`
//! tokens of a case file, and the small write/remove/mkdir/symlink/chmod/git/config
//! vocabulary that puts a sandbox in a case's state.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde_json::{Map, Value};

use crate::golden_gates_model::{Action, Host, Res};
use crate::golden_gates_sandbox::{Sandbox, write_at};

/// `rel` under `base`, refusing anything that climbs out of it.
fn inside(base: &Path, rel: &str) -> Res<PathBuf> {
  let escape = || format!("fixture path escapes sandbox: {rel}");
  let mut out = base.to_path_buf();
  for part in Path::new(rel).components() {
    match part {
      Component::Normal(name) => out.push(name),
      Component::CurDir => {}
      Component::ParentDir if out != base && out.pop() => {}
      Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
        return Err(escape().into());
      }
    }
  }
  Ok(out)
}

fn path_text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

fn single(map: &Map<String, Value>) -> Option<(&String, &Value)> {
  let mut entries = map.iter();
  match (entries.next(), entries.next()) {
    (Some(only), None) => Some(only),
    _ => None,
  }
}

impl Sandbox {
  /// A path field with its token expanded; the result is inside the sandbox.
  pub(crate) fn resolve(&self, path: &str, host: Host) -> Res<PathBuf> {
    let state = self.project.join(host.state_dir());
    let tokens = [
      ("$PROJECT", &self.project),
      ("$ROOT", &self.root),
      ("$HOME", &self.home),
      ("$HOST_STATE", &state),
    ];
    for (token, base) in tokens {
      if path == token {
        return Ok(base.clone());
      }
      if let Some(rest) = path.strip_prefix(token).and_then(|r| r.strip_prefix('/')) {
        return inside(base, rest);
      }
    }
    if path.starts_with('$') {
      return Err(format!("unknown path token: {path}").into());
    }
    inside(&self.project, path)
  }

  fn expand(&self, template: &str, host: Host) -> Res<String> {
    let token = Regex::new(r"\$[A-Z][A-Z_]*\b")?;
    let mut out = String::new();
    let mut last = 0;
    for found in token.find_iter(template) {
      out.push_str(template.get(last..found.start()).unwrap_or_default());
      out.push_str(&path_text(&self.resolve(found.as_str(), host)?));
      last = found.end();
    }
    out.push_str(template.get(last..).unwrap_or_default());
    Ok(out)
  }

  /// `value` with its tagged `$path` and `$template` objects expanded, all else unchanged.
  pub(crate) fn materialize(&self, value: &Value, host: Host) -> Res<Value> {
    match value {
      Value::Array(items) => Ok(Value::Array(
        items
          .iter()
          .map(|item| self.materialize(item, host))
          .collect::<Res<Vec<_>>>()?,
      )),
      Value::Object(map) => self.materialize_object(map, host),
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(value.clone()),
    }
  }

  fn materialize_object(&self, map: &Map<String, Value>, host: Host) -> Res<Value> {
    if let Some((key, inner)) = single(map).filter(|(key, _)| key.starts_with('$')) {
      let text = inner
        .as_str()
        .ok_or_else(|| format!("{key} must be a string"))?;
      return match key.as_str() {
        "$path" => Ok(Value::String(path_text(&self.resolve(text, host)?))),
        "$template" => Ok(Value::String(self.expand(text, host)?)),
        _ => Err(format!("unknown tag: {key}").into()),
      };
    }
    let mut out = Map::new();
    for (key, item) in map {
      out.insert(key.clone(), self.materialize(item, host)?);
    }
    Ok(Value::Object(out))
  }

  /// `value` materialized to a string.
  pub(crate) fn materialize_str(&self, value: &Value, host: Host) -> Res<String> {
    let expanded = self.materialize(value, host)?;
    expanded
      .as_str()
      .map(str::to_owned)
      .ok_or_else(|| format!("expected a string, got {expanded}").into())
  }

  /// A path to mutate: inside the sandbox, with no symlink on the way.
  fn mutation_path(&self, path: &str, host: Host, final_link: bool) -> Res<PathBuf> {
    let abs = self.resolve(path, host)?;
    let back = abs
      .strip_prefix(&self.root)
      .map_err(|_outside| format!("fixture path escapes sandbox: {path}"))?;
    let parts: Vec<Component<'_>> = back.components().collect();
    let mut current = self.root.clone();
    for (index, part) in parts.iter().enumerate() {
      current.push(part);
      let is_link = match fs::symlink_metadata(&current) {
        Ok(meta) => meta.file_type().is_symlink(),
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
      };
      if is_link && !(final_link && index + 1 == parts.len()) {
        return Err(format!("fixture mutation follows symlink: {path}").into());
      }
    }
    Ok(abs)
  }

  /// Run `actions` against this sandbox.
  pub(crate) fn apply(&self, actions: &[Action], host: Host) -> Res<()> {
    actions
      .iter()
      .try_for_each(|action| self.apply_one(action, host))
  }

  fn apply_one(&self, action: &Action, host: Host) -> Res<()> {
    match action {
      Action::Write { path, body } => {
        let target = self.mutation_path(path, host, false)?;
        write_at(&target, &self.materialize_str(body, host)?)
      }
      Action::Remove { path } => remove(&self.mutation_path(path, host, true)?),
      Action::Mkdir { path } => Ok(fs::create_dir_all(self.mutation_path(path, host, false)?)?),
      Action::Symlink { path, target } => {
        let link = self.mutation_path(path, host, false)?;
        if let Some(parent) = link.parent() {
          fs::create_dir_all(parent)?;
        }
        Ok(symlink(self.resolve(&target.path, host)?, link)?)
      }
      Action::Chmod { path, mode } => {
        let bits = u32::from_str_radix(mode, 8)?;
        let target = self.mutation_path(path, host, false)?;
        Ok(fs::set_permissions(
          target,
          fs::Permissions::from_mode(bits),
        )?)
      }
      Action::Git { args } => {
        let words = args
          .iter()
          .map(|arg| self.materialize_str(arg, host))
          .collect::<Res<Vec<String>>>()?;
        self.git(&words).map(drop)
      }
      Action::Config { host, scope, body } => self.write_config(*host, *scope, body),
    }
  }
}

fn remove(path: &Path) -> Res<()> {
  let meta = match fs::symlink_metadata(path) {
    Ok(meta) => meta,
    Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
    Err(error) => return Err(error.into()),
  };
  if meta.is_dir() {
    fs::remove_dir_all(path)?;
  } else {
    fs::remove_file(path)?;
  }
  Ok(())
}
