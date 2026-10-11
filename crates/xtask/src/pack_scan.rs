//! Bun import scan and path helpers for the opencode tarball closure.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::ci_yaml::bun_binary;
use crate::pack_npm::PackedFile;

pub(crate) struct Scanned {
  pub(crate) builtins: HashSet<String>,
  pub(crate) by_file: HashMap<String, Vec<String>>,
}

pub(crate) fn scan_imports(package_dir: &Path, files: &[PackedFile]) -> Result<Scanned, String> {
  let mut body = Vec::new();
  for file in files {
    if !has_ext(&file.path, "ts") && !has_ext(&file.path, "js") {
      continue;
    }
    let source = blank_shebang(&read_file(&package_dir.join(&file.path))?);
    let loader = if has_ext(&file.path, "ts") {
      "ts"
    } else {
      "js"
    };
    body.push(serde_json::json!({ "path": file.path, "loader": loader, "source": source }));
  }
  if body.is_empty() {
    return Ok(Scanned {
      builtins: HashSet::new(),
      by_file: HashMap::new(),
    });
  }
  let payload = serde_json::json!({ "files": body }).to_string();
  let value = bun_scan(&payload)?;
  let builtins = value
    .get("builtins")
    .and_then(Value::as_array)
    .map(|items| {
      items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
    })
    .unwrap_or_default();
  let mut by_file = HashMap::new();
  let imports = value.get("imports").and_then(Value::as_object);
  for (path, found) in imports.into_iter().flatten() {
    if let Some(message) = found.get("error").and_then(Value::as_str) {
      return Err(format!("pack: scan {path}: {message}"));
    }
    let specs = found
      .as_array()
      .map(|items| {
        items
          .iter()
          .filter_map(|item| item.as_str().map(str::to_owned))
          .collect()
      })
      .unwrap_or_default();
    by_file.insert(path.clone(), specs);
  }
  Ok(Scanned { builtins, by_file })
}

fn bun_scan(payload: &str) -> Result<Value, String> {
  let script = "const fs=require('fs');const {builtinModules}=require('node:module');const body=JSON.parse(fs.readFileSync(0,'utf8'));const imports={};for (const file of body.files){try{imports[file.path]=new Bun.Transpiler({loader:file.loader}).scanImports(file.source).map(item=>item.path);}catch(err){imports[file.path]={error:String(err&&err.message||err)};}}process.stdout.write(JSON.stringify({builtins:builtinModules,imports}));";
  let mut child = Command::new(bun_binary()?)
    .args(["-e", script])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| format!("pack: cannot run bun: {err}"))?;
  {
    let mut stdin = child.stdin.take().ok_or("pack: bun stdin is closed")?;
    stdin
      .write_all(payload.as_bytes())
      .map_err(|err| format!("pack: cannot write sources to bun: {err}"))?;
  }
  let output = child
    .wait_with_output()
    .map_err(|err| format!("pack: bun failed: {err}"))?;
  if !output.status.success() {
    return Err(format!(
      "pack: bun scan failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  serde_json::from_slice(&output.stdout).map_err(|err| format!("pack: bun scan JSON: {err}"))
}

pub(crate) fn packs(paths: &HashSet<String>, path: &str) -> bool {
  if let Some(dir) = path.strip_suffix('/') {
    return path == "/"
      || paths
        .iter()
        .any(|candidate| candidate.starts_with(path) || candidate == dir);
  }
  paths.contains(path)
}

pub(crate) fn root_dir(variable: &str) -> String {
  if variable == "TOOLU_OPENCODE_ROOT" {
    return String::new();
  }
  if variable == "TOOLU_PLUGIN_ROOT" {
    return "plugins/toolu/".to_owned();
  }
  let plugin = variable
    .strip_prefix("TOOLU_PLUGIN_ROOT_")
    .unwrap_or(variable)
    .to_ascii_lowercase()
    .replace('_', "-");
  format!("plugins/{plugin}/")
}

pub(crate) fn dirname(path: &str) -> &str {
  match path.rsplit_once('/') {
    Some(("", _)) => "/",
    Some((dir, _)) => dir,
    None => ".",
  }
}

pub(crate) fn normalize(path: &str) -> String {
  let absolute = path.starts_with('/');
  let trailing = path.ends_with('/') && path != "/";
  let mut out = Vec::new();
  for part in path.split('/') {
    if part.is_empty() || part == "." {
      continue;
    }
    if part == ".." {
      if absolute && out.is_empty() {
        continue;
      }
      if out.last().copied() == Some("..") || out.is_empty() {
        out.push("..");
      } else {
        out.pop();
      }
    } else {
      out.push(part);
    }
  }
  let mut joined = if absolute {
    format!("/{}", out.join("/"))
  } else {
    out.join("/")
  };
  if joined.is_empty() {
    joined = if absolute {
      "/".to_owned()
    } else {
      ".".to_owned()
    };
  }
  if trailing && !joined.ends_with('/') {
    joined.push('/');
  }
  joined
}

pub(crate) fn scheme(target: &str) -> bool {
  regex::Regex::new(r"(?i)^[a-z][a-z0-9+.-]*:").is_ok_and(|expr| expr.is_match(target))
}

pub(crate) fn split_bare(specifier: &str) -> (String, String) {
  let parts = specifier.split('/').collect::<Vec<_>>();
  let size = if specifier.starts_with('@') { 2 } else { 1 };
  let pkg = parts
    .iter()
    .take(size)
    .copied()
    .collect::<Vec<_>>()
    .join("/");
  let rest = parts
    .iter()
    .skip(size)
    .copied()
    .collect::<Vec<_>>()
    .join("/");
  let key = if rest.is_empty() {
    ".".to_owned()
  } else {
    format!("./{rest}")
  };
  (pkg, key)
}

pub(crate) fn is_builtin(specifier: &str, builtins: &HashSet<String>) -> bool {
  specifier.starts_with("node:")
    || specifier.starts_with("bun:")
    || specifier == "bun"
    || builtins.contains(specifier)
}

pub(crate) fn text_file(path: &str) -> bool {
  has_ext(path, "md") || has_ext(path, "json") || has_ext(path, "ts") || has_ext(path, "js")
}

pub(crate) fn has_ext(path: &str, want: &str) -> bool {
  Path::new(path)
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case(want))
}

fn blank_shebang(text: &str) -> String {
  let Some(rest) = text.strip_prefix("#!") else {
    return text.to_owned();
  };
  match rest.find('\n') {
    Some(index) => rest.get(index..).unwrap_or("").to_owned(),
    None => String::new(),
  }
}

pub(crate) fn read_file(path: &Path) -> Result<String, String> {
  std::fs::read_to_string(path)
    .map_err(|err| format!("pack: cannot read {}: {err}", path.display()))
}

pub(crate) fn unique(items: Vec<String>) -> Vec<String> {
  let mut seen = HashSet::new();
  let mut out = Vec::new();
  for item in items {
    if seen.insert(item.clone()) {
      out.push(item);
    }
  }
  out
}

#[cfg(test)]
#[path = "tests/pack_scan_test.rs"]
mod tests;
