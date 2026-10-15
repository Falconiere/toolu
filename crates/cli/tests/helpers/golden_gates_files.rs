//! The files a hook wrote: a snapshot of the sandbox before the hook and after it, and
//! the difference, with sandbox paths and timestamps normalized.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use regex::Regex;

use crate::golden_gates_model::Res;
use crate::golden_gates_run::normalize;

/// Relative path to body.
pub(crate) type Files = BTreeMap<String, String>;

/// Paths under the sandbox root that are not hook state. As in the TypeScript harness,
/// `home` is skipped whole: only its `.claude` and `.codex` could be state, and the walk
/// never reaches them.
fn ignored(rel: &Path) -> bool {
  let parts: Vec<String> = rel
    .iter()
    .map(|part| part.to_string_lossy().into_owned())
    .collect();
  let first = parts.first().map_or("", String::as_str);
  let second = parts.get(1).map(String::as_str);
  parts.iter().any(|part| part == ".git")
    || first == "settings"
    || first.starts_with("bin-")
    || (first == "home" && second != Some(".claude") && second != Some(".codex"))
}

fn walk(root: &Path, dir: &Path, out: &mut Files) -> Res<()> {
  for entry in fs::read_dir(dir)? {
    let path = entry?.path();
    let rel = path.strip_prefix(root)?;
    if ignored(rel) {
      continue;
    }
    let meta = match fs::metadata(&path) {
      Ok(meta) => meta,
      Err(error) if error.kind() == ErrorKind::NotFound => continue,
      Err(error) => return Err(error.into()),
    };
    if meta.is_dir() {
      walk(root, &path, out)?;
    } else if meta.is_file() {
      let body = String::from_utf8_lossy(&fs::read(&path)?).into_owned();
      out.insert(rel.to_string_lossy().into_owned(), body);
    }
  }
  Ok(())
}

/// Every file under `root` that could be hook state.
pub(crate) fn snapshot(root: &Path) -> Res<Files> {
  let mut out = Files::new();
  walk(root, root, &mut out)?;
  Ok(out)
}

/// What changed from `before` to `after`: the new body (times as `<time>`), `None` for a removal.
pub(crate) fn touched(
  before: &Files,
  after: &Files,
  root: &Path,
) -> Res<BTreeMap<String, Option<String>>> {
  let time = Regex::new(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ")?;
  let mut files = BTreeMap::new();
  for (rel, body) in after {
    if before.get(rel) != Some(body) {
      let text = time
        .replace_all(&normalize(body, root), "<time>")
        .into_owned();
      files.insert(rel.clone(), Some(text));
    }
  }
  for rel in before.keys().filter(|rel| !after.contains_key(*rel)) {
    files.insert(rel.clone(), None);
  }
  Ok(files)
}
