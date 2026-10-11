//! Closure of the published `@toolu/opencode` tarball.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use regex::Regex;
use serde_json::Value;

use crate::pack_npm::PackedFile;
use crate::pack_scan::{
  dirname, has_ext, is_builtin, normalize, packs, read_file, root_dir, scan_imports, scheme,
  split_bare, text_file, unique,
};

const NAME: &str = "@toolu/opencode";
const SUFFIXES: [&str; 5] = ["", ".ts", ".js", "/index.ts", "/index.js"];

/// Every closure problem, one line each.
pub(crate) fn closure_problems(
  package_dir: &Path,
  files: &[PackedFile],
  source_plugins: &Path,
  core_exports: &[String],
) -> Result<Vec<String>, String> {
  let paths = files
    .iter()
    .map(|file| file.path.clone())
    .collect::<HashSet<_>>();
  let manifest = manifest(package_dir)?;
  let imports = scan_imports(package_dir, files)?;
  let builtins = imports.builtins;
  let mut problems = Vec::new();
  for file in files {
    if !text_file(&file.path) {
      continue;
    }
    let text = read_file(&package_dir.join(&file.path))?;
    let context = Context {
      paths: &paths,
      manifest: &manifest,
      core_exports,
      builtins: &builtins,
      imports: &imports.by_file,
    };
    problems.extend(file_problems(&file.path, &text, &context)?);
  }
  problems.extend(export_problems(&manifest, &paths));
  problems.extend(mode_problems(files, source_plugins));
  Ok(unique(problems))
}

struct Manifest {
  main: Option<String>,
  exports: Vec<(String, String)>,
  dependencies: HashSet<String>,
}

struct Context<'a> {
  paths: &'a HashSet<String>,
  manifest: &'a Manifest,
  core_exports: &'a [String],
  builtins: &'a HashSet<String>,
  imports: &'a HashMap<String, Vec<String>>,
}

fn file_problems(file: &str, text: &str, context: &Context<'_>) -> Result<Vec<String>, String> {
  let surface = file.starts_with("generated/") || file.starts_with("plugins/");
  let mut problems = Vec::new();
  if surface && (has_ext(file, "md") || has_ext(file, "json")) {
    problems.extend(reference_problems(file, text, context.paths)?);
  }
  if file.starts_with("generated/") && has_ext(file, "md") {
    problems.extend(link_problems(file, text, context.paths)?);
  }
  if has_ext(file, "ts") || has_ext(file, "js") {
    problems.extend(import_problems(file, context));
  }
  Ok(problems)
}

fn import_problems(file: &str, context: &Context<'_>) -> Vec<String> {
  let Some(found) = context.imports.get(file) else {
    return Vec::new();
  };
  found
    .iter()
    .filter_map(|specifier| import_problem(file, specifier, context))
    .collect()
}

fn reference_problems(
  file: &str,
  text: &str,
  paths: &HashSet<String>,
) -> Result<Vec<String>, String> {
  let expr =
    Regex::new(r#"\$\{?(TOOLU_PLUGIN_ROOT(?:_[A-Z0-9_]+)?|TOOLU_OPENCODE_ROOT)\}?/([^\s`"')\]]*)"#)
      .map_err(|err| format!("pack: reference pattern: {err}"))?;
  let placeholder =
    Regex::new(r"[<…*${]").map_err(|err| format!("pack: placeholder pattern: {err}"))?;
  let mut problems = Vec::new();
  for caps in expr.captures_iter(text) {
    let Some(whole) = caps.get(0).map(|item| item.as_str()) else {
      continue;
    };
    let variable = caps.get(1).map_or("", |item| item.as_str());
    let raw = caps.get(2).map_or("", |item| item.as_str());
    let path = raw.trim_end_matches(['.', ',', ';', ':']);
    if path.is_empty() || placeholder.is_match(path) {
      continue;
    }
    let resolved = normalize(&format!("{}{path}", root_dir(variable)));
    if !packs(paths, &resolved) {
      problems.push(format!(
        "{NAME}: {file} references {whole}, which the tarball does not contain"
      ));
    }
  }
  Ok(problems)
}

fn link_problems(file: &str, text: &str, paths: &HashSet<String>) -> Result<Vec<String>, String> {
  let expr = Regex::new(r"\]\(([^)\s]+)\)").map_err(|err| format!("pack: link pattern: {err}"))?;
  let mut problems = Vec::new();
  for caps in expr.captures_iter(text) {
    let link = caps.get(1).map_or("", |item| item.as_str());
    let target = link.split('#').next().unwrap_or(link);
    if target.is_empty() || target.starts_with('/') || scheme(target) {
      continue;
    }
    let resolved = normalize(&format!("{}/{target}", dirname(file)));
    if !paths.contains(&resolved) {
      problems.push(format!(
        "{NAME}: {file} links {link}, which the tarball does not contain"
      ));
    }
  }
  Ok(problems)
}

fn import_problem(file: &str, specifier: &str, context: &Context<'_>) -> Option<String> {
  if specifier.starts_with('.') {
    let base = normalize(&format!("{}/{specifier}", dirname(file)));
    if SUFFIXES
      .iter()
      .any(|suffix| context.paths.contains(&format!("{base}{suffix}")))
    {
      return None;
    }
    return Some(format!(
      "{NAME}: {file} imports {specifier}, which the tarball does not contain"
    ));
  }
  if is_builtin(specifier, context.builtins) {
    return None;
  }
  let (pkg, key) = split_bare(specifier);
  if !context.manifest.dependencies.contains(&pkg) {
    return Some(format!(
      "{NAME}: {file} imports {specifier}, which is not a declared dependency"
    ));
  }
  if pkg == "@toolu/core" && !context.core_exports.iter().any(|export| export == &key) {
    return Some(format!(
      "{NAME}: {file} imports {specifier}, which @toolu/core does not export"
    ));
  }
  None
}

fn export_problems(manifest: &Manifest, paths: &HashSet<String>) -> Vec<String> {
  let mut targets = manifest.exports.clone();
  if let Some(main) = &manifest.main {
    targets.push(("main".to_owned(), main.clone()));
  }
  targets
    .into_iter()
    .filter(|(_, target)| !paths.contains(&normalize(target)))
    .map(|(key, target)| format!("{NAME}: export {key} → {target} is not in the tarball"))
    .collect()
}

fn mode_problems(files: &[PackedFile], source_plugins: &Path) -> Vec<String> {
  let mut problems = Vec::new();
  for file in files {
    if !file.path.starts_with("plugins/") || file.mode & 0o111 != 0 {
      continue;
    }
    let Some(rel) = file.path.strip_prefix("plugins/") else {
      continue;
    };
    let source = source_plugins.join(rel);
    let Ok(meta) = std::fs::metadata(&source) else {
      continue;
    };
    if meta.permissions().mode() & 0o111 == 0 {
      continue;
    }
    let octal = format!("{:o}", file.mode & 0o777);
    problems.push(format!(
      "{NAME}: {} lost its executable bit (mode {octal})",
      file.path
    ));
  }
  problems
}

fn manifest(package_dir: &Path) -> Result<Manifest, String> {
  let path = package_dir.join("package.json");
  let text = read_file(&path)?;
  let doc: Value =
    serde_json::from_str(&text).map_err(|err| format!("pack: {}: {err}", path.display()))?;
  let exports = doc
    .get("exports")
    .and_then(Value::as_object)
    .map(|entries| {
      entries
        .iter()
        .filter_map(|(key, value)| {
          value
            .as_str()
            .map(|target| (key.clone(), target.to_owned()))
        })
        .collect()
    })
    .unwrap_or_default();
  let dependencies = doc
    .get("dependencies")
    .and_then(Value::as_object)
    .map(|entries| entries.keys().cloned().collect())
    .unwrap_or_default();
  let main = doc.get("main").and_then(Value::as_str).map(str::to_owned);
  Ok(Manifest {
    main,
    exports,
    dependencies,
  })
}

#[cfg(test)]
#[path = "tests/pack_closure_test.rs"]
mod tests;
