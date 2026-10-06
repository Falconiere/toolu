//! The 35 layouts of `detect-project.test.ts`, as data in `fixtures/detect_layouts.json`,
//! each built in a real git repository named `project`.

use std::os::unix::fs::symlink;
use std::path::PathBuf;

use serde_json::Value;

use super::scratch::{Res, Scratch, mkdir, write};

/// One layout and what TypeScript answers for it.
pub(crate) struct Layout {
  pub(crate) name: String,
  /// Files committed to the repository.
  tracked: Vec<(String, String)>,
  /// Files written but never added.
  untracked: Vec<(String, String)>,
  /// Directories created at the root.
  dirs: Vec<String>,
  /// Symlinks at the root, name and target.
  links: Vec<(String, String)>,
  /// The probe index and answer the bats suite asserts, when it does.
  pub(crate) bats: Option<(usize, String)>,
  /// Node package manager, rust, python, ts, ts linter, python linter, clippy.
  pub(crate) expect: Vec<String>,
}

fn text(value: &Value) -> Res<String> {
  value
    .as_str()
    .map(str::to_owned)
    .ok_or_else(|| format!("not text: {value}"))
}

fn pairs(layout: &Value, key: &str) -> Res<Vec<(String, String)>> {
  let Some(map) = layout.get(key).and_then(Value::as_object) else {
    return Ok(Vec::new());
  };
  map
    .iter()
    .map(|(name, body)| Ok((name.clone(), text(body)?)))
    .collect()
}

fn strings(layout: &Value, key: &str) -> Res<Vec<String>> {
  let items = layout
    .get(key)
    .and_then(Value::as_array)
    .map_or(&[][..], Vec::as_slice);
  items.iter().map(text).collect()
}

fn bats(layout: &Value) -> Res<Option<(usize, String)>> {
  let Some(pair) = layout.get("bats").and_then(Value::as_array) else {
    return Ok(None);
  };
  let index = pair.first().and_then(Value::as_u64).ok_or("bats index")?;
  let answer = text(pair.get(1).ok_or("bats answer")?)?;
  Ok(Some((
    usize::try_from(index).map_err(|err| err.to_string())?,
    answer,
  )))
}

fn layout(value: &Value) -> Res<Layout> {
  Ok(Layout {
    name: text(value.get("name").ok_or("layout without a name")?)?,
    tracked: pairs(value, "tracked")?,
    untracked: pairs(value, "untracked")?,
    dirs: strings(value, "dirs")?,
    links: pairs(value, "links")?,
    bats: bats(value)?,
    expect: strings(value, "expect")?,
  })
}

/// Every layout of the fixture file.
pub(crate) fn load() -> Res<Vec<Layout>> {
  let file = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/detect_layouts.json"
  );
  let text = std::fs::read_to_string(file).map_err(|err| format!("{file}: {err}"))?;
  let doc: Value = serde_json::from_str(&text).map_err(|err| format!("{file}: {err}"))?;
  let layouts = doc
    .get("layouts")
    .and_then(Value::as_array)
    .ok_or("no layouts array")?;
  layouts.iter().map(layout).collect()
}

/// Build `layout` in a fresh repository named `project` and return its root.
pub(crate) fn build(scratch: &Scratch, layout: &Layout) -> Res<PathBuf> {
  let root = scratch.root.join("project");
  mkdir(&root)?;
  scratch.git(&root, &["init", "-q", "-b", "main"])?;
  for (rel, body) in &layout.tracked {
    write(&root.join(rel), body.as_bytes())?;
  }
  for dir in &layout.dirs {
    mkdir(&root.join(dir))?;
  }
  for (name, target) in &layout.links {
    symlink(target, root.join(name)).map_err(|err| format!("{name}: {err}"))?;
  }
  scratch.git(&root, &["add", "-A"])?;
  scratch.git(&root, &["commit", "-q", "--allow-empty", "-m", "markers"])?;
  for (rel, body) in &layout.untracked {
    write(&root.join(rel), body.as_bytes())?;
  }
  Ok(root)
}
