//! `cargo xtask dist check` and `dist checksums`: bundle drift and release sums.

use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ci_yaml::bun_binary;
use crate::options::Options;
use crate::{Verdict, output};

/// The four archives `homebrew-formula` already requires, in its order.
const ARCHIVES: [&str; 4] = [
  "toolu-darwin-arm64.tar.gz",
  "toolu-darwin-amd64.tar.gz",
  "toolu-linux-arm64.tar.gz",
  "toolu-linux-amd64.tar.gz",
];

struct Entry {
  plugin: String,
  name: String,
  source: String,
}

struct Stage {
  path: PathBuf,
}

impl Stage {
  fn new() -> Result<Self, String> {
    static TICK: AtomicU64 = AtomicU64::new(0);
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("toolu-bundles-{}-{tick}", std::process::id()));
    std::fs::create_dir(&path).map_err(|err| format!("cannot create {}: {err}", path.display()))?;
    Ok(Self { path })
  }
}

impl Drop for Stage {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.path);
  }
}

/// `dist check` or `dist checksums <dir>`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  match options.files.as_slice() {
    [mode] if mode == Path::new("check") => Ok(output::findings("dist", &drift(&options.root)?)),
    [mode, dir] if mode == Path::new("checksums") => Ok(output::findings("dist", &checksums(dir)?)),
    _ => Err("usage: cargo xtask dist check | cargo xtask dist checksums <dir>".to_owned()),
  }
}

/// Drift, missing bundles, and orphans. A missing `bun` is an error.
fn drift(root: &Path) -> Result<Vec<String>, String> {
  drift_with(root, &bun_binary()?)
}

fn drift_with(root: &Path, bun: &Path) -> Result<Vec<String>, String> {
  let stage = Stage::new()?;
  let entries = discover(root)?;
  for entry in &entries {
    let outfile = stage
      .path
      .join(&entry.plugin)
      .join(format!("{}.js", entry.name));
    let parent = outfile
      .parent()
      .ok_or_else(|| format!("no parent for {}", outfile.display()))?;
    std::fs::create_dir_all(parent)
      .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
    build_one(bun, root, &entry.source, &outfile)?;
  }
  compare(root, &stage, &entries)
}

fn compare(root: &Path, stage: &Stage, entries: &[Entry]) -> Result<Vec<String>, String> {
  let mut problems = Vec::new();
  let mut wanted = Vec::new();
  for entry in entries {
    let path = format!("plugins/{}/hooks/dist/{}.js", entry.plugin, entry.name);
    wanted.push(path.clone());
    let committed = root.join(&path);
    let staged = stage
      .path
      .join(&entry.plugin)
      .join(format!("{}.js", entry.name));
    if !committed.is_file() {
      problems.push((path, "missing"));
    } else if !same_bundle(&committed, &staged)? {
      problems.push((path, "drift"));
    }
  }
  for path in committed_bundles(root)? {
    if !wanted.iter().any(|item| item == &path) {
      problems.push((path, "orphan"));
    }
  }
  problems.sort();
  Ok(
    problems
      .into_iter()
      .map(|(path, kind)| format!("RED  {kind} {path}"))
      .collect(),
  )
}

fn discover(root: &Path) -> Result<Vec<Entry>, String> {
  let mut entries = Vec::new();
  for plugin in child_names(&root.join("plugins"), true)? {
    let src = root.join("plugins").join(&plugin).join("hooks/src");
    for file in child_names(&src, false)? {
      if !is_entry(&file) {
        continue;
      }
      let name = Path::new(&file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&file)
        .to_owned();
      entries.push(Entry {
        source: format!("plugins/{plugin}/hooks/src/{file}"),
        plugin: plugin.clone(),
        name,
      });
    }
  }
  Ok(entries)
}

fn committed_bundles(root: &Path) -> Result<Vec<String>, String> {
  let mut paths = Vec::new();
  for plugin in child_names(&root.join("plugins"), true)? {
    let dist = root.join("plugins").join(&plugin).join("hooks/dist");
    for file in child_names(&dist, false)? {
      if Path::new(&file)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
      {
        paths.push(format!("plugins/{plugin}/hooks/dist/{file}"));
      }
    }
  }
  Ok(paths)
}

fn is_entry(name: &str) -> bool {
  let path = Path::new(name);
  let Some(ext) = path.extension() else {
    return false;
  };
  if !ext.eq_ignore_ascii_case("ts") {
    return false;
  }
  let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
    return false;
  };
  !suffix(stem, "test") && !suffix(stem, "d")
}

fn suffix(stem: &str, ext: &str) -> bool {
  Path::new(stem)
    .extension()
    .is_some_and(|found| found.eq_ignore_ascii_case(ext))
}

fn child_names(dir: &Path, directories: bool) -> Result<Vec<String>, String> {
  let read = match std::fs::read_dir(dir) {
    Ok(read) => read,
    Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(format!("cannot list {}: {err}", dir.display())),
  };
  let mut names = Vec::new();
  for entry in read {
    let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
    let kind = entry
      .file_type()
      .map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
    if kind.is_dir() == directories {
      names.push(entry.file_name().to_string_lossy().into_owned());
    }
  }
  names.sort();
  Ok(names)
}

fn build_one(bun: &Path, root: &Path, source: &str, outfile: &Path) -> Result<(), String> {
  let output = Command::new(bun)
    .arg("build")
    .arg(source)
    .args(["--target", "bun", "--format", "esm", "--sourcemap=none"])
    .arg("--outfile")
    .arg(outfile)
    .current_dir(root)
    .output()
    .map_err(|err| bun_spawn_error(&err, bun))?;
  if output.status.success() {
    return Ok(());
  }
  let detail = String::from_utf8_lossy(&output.stderr);
  Err(format!("bun build failed for {source}: {}", detail.trim()))
}

fn bun_spawn_error(err: &std::io::Error, bun: &Path) -> String {
  if err.kind() == ErrorKind::NotFound {
    return format!("bun is not installed ({})", bun.display());
  }
  format!("cannot run bun: {err}")
}

fn same_bundle(committed: &Path, staged: &Path) -> Result<bool, String> {
  let staged_bytes =
    std::fs::read(staged).map_err(|err| format!("cannot read {}: {err}", staged.display()))?;
  let committed_bytes = std::fs::read(committed)
    .map_err(|err| format!("cannot read {}: {err}", committed.display()))?;
  if staged_bytes != committed_bytes {
    return Ok(false);
  }
  if staged_bytes.starts_with(b"#!") {
    return executable(committed);
  }
  Ok(true)
}

fn executable(path: &Path) -> Result<bool, String> {
  let meta =
    std::fs::metadata(path).map_err(|err| format!("cannot stat {}: {err}", path.display()))?;
  Ok(meta.permissions().mode() & 0o111 != 0)
}

/// Findings for missing archives. Writes `SHA256SUMS` only when all four exist.
fn checksums(dir: &Path) -> Result<Vec<String>, String> {
  let missing: Vec<&str> = ARCHIVES
    .into_iter()
    .filter(|name| !dir.join(name).is_file())
    .collect();
  if !missing.is_empty() {
    return Ok(
      missing
        .into_iter()
        .map(|name| format!("missing {name}"))
        .collect(),
    );
  }
  let mut body = String::new();
  for name in ARCHIVES {
    let hex = sha256(&dir.join(name))?;
    let _ = std::fmt::Write::write_fmt(&mut body, format_args!("{hex}  {name}\n"));
  }
  let dest = dir.join("SHA256SUMS");
  let tmp = dir.join(".SHA256SUMS.tmp");
  std::fs::write(&tmp, body).map_err(|err| format!("cannot write {}: {err}", tmp.display()))?;
  std::fs::rename(&tmp, &dest).map_err(|err| format!("cannot write {}: {err}", dest.display()))?;
  Ok(Vec::new())
}

fn sha256(path: &Path) -> Result<String, String> {
  if let Some(hex) = digest_command("sha256sum", &[], path)? {
    return Ok(hex);
  }
  if let Some(hex) = digest_command("shasum", &["-a", "256"], path)? {
    return Ok(hex);
  }
  Err("sha256sum is not installed".to_owned())
}

fn digest_command(program: &str, prefix: &[&str], path: &Path) -> Result<Option<String>, String> {
  match Command::new(program).args(prefix).arg(path).output() {
    Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
    Err(err) => Err(format!("{program}: {err}")),
    Ok(output) if output.status.success() => Ok(Some(hex_token(&output.stdout)?)),
    Ok(output) => Err(format!(
      "{program} failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    )),
  }
}

fn hex_token(stdout: &[u8]) -> Result<String, String> {
  let text = String::from_utf8_lossy(stdout);
  let Some(token) = text.split_whitespace().next() else {
    return Err("sha256: empty output".to_owned());
  };
  if token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return Ok(token.to_ascii_lowercase());
  }
  Err(format!("sha256: unexpected output {token}"))
}

#[cfg(test)]
#[path = "tests/dist_test.rs"]
mod tests;
