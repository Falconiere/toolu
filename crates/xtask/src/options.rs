//! Command-line options shared by every task.

use std::path::PathBuf;

/// The repository this xtask was built from: `crates/xtask/../..`.
const DEFAULT_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Parsed options. Each task reads the ones it needs.
#[derive(Debug, Default)]
pub(crate) struct Options {
  /// The workspace root (`--root`, default: this repository).
  pub(crate) root: PathBuf,
  /// The base revision for `check-gate-change` (`--base`).
  pub(crate) base: Option<String>,
  /// The pull request title for `check-gate-change` (`--title`).
  pub(crate) title: Option<String>,
  /// Gate steps to run instead of all of them (`--only`, repeatable).
  pub(crate) only: Vec<String>,
  /// Positional arguments (the llvm-cov JSON file of `check-coverage`).
  pub(crate) files: Vec<PathBuf>,
  /// Where `measure` writes its report (`--out`).
  pub(crate) out: Option<PathBuf>,
  /// The command `measure` runs: every word after `--`.
  pub(crate) command: Vec<String>,
}

impl Options {
  /// Parse `args` (the words after the task name).
  pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
    let mut options = Options::default();
    let mut root = None;
    let mut rest = args.iter();
    while let Some(word) = rest.next() {
      if word == "--" {
        options.command = rest.by_ref().cloned().collect();
        break;
      }
      if !word.starts_with("--") {
        options.files.push(PathBuf::from(word));
        continue;
      }
      let value = rest
        .next()
        // The separator is never a value: `--out -- cmd` lacks the file.
        .filter(|value| value.as_str() != "--")
        .ok_or_else(|| format!("{word} needs a value"))?
        .clone();
      match word.as_str() {
        "--root" => root = Some(PathBuf::from(value)),
        "--base" => options.base = Some(value),
        "--title" => options.title = Some(value),
        "--only" => options.only.push(value),
        "--out" => options.out = Some(PathBuf::from(value)),
        _ => return Err(format!("unknown option {word}")),
      }
    }
    let root = root.unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT));
    options.root = std::fs::canonicalize(&root)
      .map_err(|err| format!("cannot resolve root {}: {err}", root.display()))?;
    Ok(options)
  }

  /// Refuse `measure`'s options on any other task.
  pub(crate) fn for_task(self, task: &str) -> Result<Self, String> {
    if task != "measure" && (self.out.is_some() || !self.command.is_empty()) {
      return Err("only measure takes --out and a command after --".to_owned());
    }
    Ok(self)
  }
}

#[cfg(test)]
#[path = "tests/options_test.rs"]
mod tests;
