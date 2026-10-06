//! Stable-path publishing (`publish.ts`). A host exports its plugin root to
//! hook processes only, never to the agent's shell, so a skill cannot name a
//! file inside the plugin. A `SessionStart` hook links it at
//! `<config root>/<dir>/<name>` instead. The path is owned only when it is
//! absent or a symlink: a regular file or a directory there is the user's and
//! is never touched.

use std::path::{Path, PathBuf};

use super::report::{HelperStatus, StartupRecord, report};
use crate::config::load::is_file;
use crate::host::roots::Roots;

/// What to publish where.
#[derive(Debug, Clone, Copy)]
pub struct PublishOptions<'a> {
  /// The plugin, the prefix of the warning line.
  pub plugin: &'a str,
  /// The absolute path of the file to publish.
  pub source: &'a Path,
  /// The directory under the config root, e.g. `jev`.
  pub dir: &'a str,
  /// The published file name, e.g. `search.sh`.
  pub name: &'a str,
  /// The word in the warning line: `wrapper` or `helper`.
  pub what: &'a str,
  /// The roots whose config root holds the path.
  pub roots: &'a Roots,
}

/// How a publish ended, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishResult {
  /// The outcome.
  pub status: HelperStatus,
  /// The published path, or the directory that could not be created; `None`
  /// when the source is missing.
  pub path: Option<PathBuf>,
}

/// A publish, with the lines its caller prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
  /// What happened.
  pub result: PublishResult,
  /// `<plugin>: cannot create <dir> — <what> not published`.
  pub warning: Option<String>,
  /// The startup report could not be written; the caller exits 1 at the end.
  pub report_error: Option<String>,
}

/// A fresh symlink beside `path`, renamed over it: readers never see the path missing.
fn relink(source: &Path, path: &Path) -> bool {
  let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
  let mut tmp = path.as_os_str().to_owned();
  tmp.push(format!(
    ".{}.{}.tmp",
    std::process::id(),
    since.map_or(0, |since| since.as_nanos())
  ));
  let tmp = PathBuf::from(tmp);
  if std::os::unix::fs::symlink(source, &tmp).is_ok() && std::fs::rename(&tmp, path).is_ok() {
    return true;
  }
  let _gone = std::fs::remove_file(&tmp);
  false
}

fn link(options: &PublishOptions<'_>) -> (PublishResult, Option<String>) {
  let done = |status, path| {
    (
      PublishResult {
        status,
        path: Some(path),
      },
      None,
    )
  };
  if !is_file(options.source) {
    return (
      PublishResult {
        status: HelperStatus::SourceMissing,
        path: None,
      },
      None,
    );
  }
  let dir = options.roots.config_root().join(options.dir);
  if std::fs::create_dir_all(&dir).is_err() {
    let warning = format!(
      "{}: cannot create {} — {} not published",
      options.plugin,
      dir.display(),
      options.what
    );
    return (
      PublishResult {
        status: HelperStatus::Unwritable,
        path: Some(dir),
      },
      Some(warning),
    );
  }
  let path = dir.join(options.name);
  match std::fs::symlink_metadata(&path) {
    Ok(meta) if !meta.file_type().is_symlink() => done(HelperStatus::KeptUserFile, path),
    Ok(_) if std::fs::read_link(&path).is_ok_and(|target| target == options.source) => {
      done(HelperStatus::Published, path)
    }
    Ok(_) | Err(_) if relink(options.source, &path) => done(HelperStatus::Published, path),
    Ok(_) | Err(_) => done(HelperStatus::LinkFailed, path),
  }
}

/// Links `source` at `<config root>/<dir>/<name>` and reports the outcome.
/// A missing source is a corrupted install and publishes nothing, silently.
pub fn publish(options: &PublishOptions<'_>) -> Published {
  let (result, warning) = link(options);
  let record = StartupRecord::Helper {
    plugin: options.plugin.to_owned(),
    source: options.source.display().to_string(),
    path: result.path.as_ref().map(|path| path.display().to_string()),
    status: result.status,
  };
  let report_error = report(options.roots.env(), &record).err();
  Published {
    result,
    warning,
    report_error,
  }
}

#[cfg(test)]
#[path = "tests/publish_test.rs"]
mod tests;
