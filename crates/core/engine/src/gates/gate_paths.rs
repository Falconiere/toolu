//! Repo-relative gate paths and Bash pathname expansion for write targets.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::pattern::Pattern;

/// Remove the root prefix only when the absolute path is inside that root.
pub(crate) fn repo_relative(path: &str, root: &Path) -> String {
  let prefix = format!("{}/", root.display());
  path.strip_prefix(&prefix).unwrap_or(path).to_owned()
}

fn globs(segment: &str) -> bool {
  let chars: Vec<char> = segment.chars().collect();
  let mut at = 0;
  while let Some(c) = chars.get(at) {
    if *c == '\\' {
      at += 2;
      continue;
    }
    if matches!(c, '*' | '?') {
      return true;
    }
    if *c == '[' && chars.get(at + 1..).is_some_and(|tail| tail.contains(&']')) {
      return true;
    }
    if matches!(c, '@' | '!' | '+') && chars.get(at + 1) == Some(&'(') {
      return true;
    }
    at += 1;
  }
  false
}

fn unescape(segment: &str) -> String {
  let mut out = String::new();
  let mut chars = segment.chars();
  while let Some(c) = chars.next() {
    if c == '\\' {
      out.push(chars.next().unwrap_or(c));
    } else {
      out.push(c);
    }
  }
  out
}

fn entries(dir: &Path) -> Vec<String> {
  let mut names = std::fs::read_dir(dir)
    .ok()
    .into_iter()
    .flatten()
    .filter_map(Result::ok)
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect::<Vec<_>>();
  names.sort();
  names
}

/// Expand each existing pathname matched by the pattern, then include the
/// literal target for Bash's no-match behavior.
pub(crate) fn expand(pattern: &str, cwd: &Path) -> Vec<String> {
  let absolute = pattern.starts_with('/');
  let shown = pattern.strip_prefix('/').unwrap_or(pattern);
  let mut found = vec![(
    Vec::<String>::new(),
    if absolute {
      PathBuf::from("/")
    } else {
      cwd.to_path_buf()
    },
  )];
  for segment in shown.split('/') {
    if !globs(segment) {
      let name = unescape(segment);
      found = found
        .into_iter()
        .map(|(mut parts, disk)| {
          parts.push(name.clone());
          (parts, disk.join(&name))
        })
        .collect();
      continue;
    }
    let compiled = Pattern::new(segment);
    found = found
      .into_iter()
      .flat_map(|(parts, disk)| {
        entries(&disk)
          .into_iter()
          .filter(|name| {
            (segment.starts_with('.') || !name.starts_with('.')) && compiled.matches(name)
          })
          .map(|name| {
            let mut next = parts.clone();
            next.push(name.clone());
            (next, disk.join(name))
          })
          .collect::<Vec<_>>()
      })
      .collect();
  }
  let mut unique = HashSet::new();
  found
    .into_iter()
    .map(|(parts, _)| format!("{}{}", if absolute { "/" } else { "" }, parts.join("/")))
    .chain(std::iter::once(pattern.to_owned()))
    .filter(|path| unique.insert(path.clone()))
    .collect()
}

#[cfg(test)]
#[path = "tests/gate_paths_test.rs"]
mod tests;
