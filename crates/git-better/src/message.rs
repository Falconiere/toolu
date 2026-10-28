//! A commit's message as git will write it, read before the command runs: `-m`
//! paragraphs, `-F` files and stdin, then `--trailer` values. Whatever only
//! exists at run time is reported, never guessed: a dynamic word, a pipe, the
//! editor, a file the same line writes or reaches through `cd`, and anything
//! but a small regular file.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use toolu_shell::analysis::ShellAnalysis;
use toolu_shell::git::{CommitArgs, MessageOrigin};
use toolu_shell::writes::write_targets;

use crate::format::{trailer_arg, with_trailers};

/// The largest `-F` file read.
pub(crate) const MAX_MESSAGE_BYTES: u64 = 64 * 1024;

/// What the rest of the line does to `-F` files.
pub(crate) struct Line {
  /// Every static path the line writes, moves or copies to.
  written: Vec<String>,
  /// The line changes directory, so a relative path may name another file.
  moves: bool,
}

impl Line {
  /// The line `analysis` read.
  pub(crate) fn of(analysis: &ShellAnalysis) -> Line {
    let written = write_targets(analysis)
      .into_iter()
      .filter_map(|target| target.path)
      .collect();
    let moves = analysis.commands.iter().any(|command| {
      let name = command.argv.first().and_then(Option::as_deref);
      matches!(name, Some("cd" | "pushd" | "popd"))
    });
    Line { written, moves }
  }

  /// The line writes `path`, or a file of the same name.
  fn writes(&self, path: &str) -> bool {
    let name = Path::new(path).file_name();
    self
      .written
      .iter()
      .any(|written| written == path || Path::new(written).file_name() == name)
  }
}

/// A `-F` file, when it can be read now and still holds the message at run time.
fn read_file(dir: &Path, path: &str, line: &Line) -> Result<String, String> {
  if line.moves {
    return Err(format!(
      "its file `{path}` is read after the line changes directory"
    ));
  }
  if line.writes(path) {
    return Err(format!("its file `{path}` is written by the same command"));
  }
  let full = dir.join(path);
  let meta = fs::metadata(&full)
    .map_err(|error| format!("its file `{path}` cannot be read yet ({error})"))?;
  if !meta.is_file() {
    return Err(format!("its file `{path}` is not a regular file"));
  }
  if meta.len() > MAX_MESSAGE_BYTES {
    return Err(format!(
      "its file `{path}` is over {} KiB",
      MAX_MESSAGE_BYTES / 1024
    ));
  }
  let mut text = String::new();
  fs::File::open(&full)
    .and_then(|file| file.take(MAX_MESSAGE_BYTES).read_to_string(&mut text))
    .map_err(|error| format!("its file `{path}` cannot be read ({error})"))?;
  Ok(text)
}

/// The commit's message as git will write it, `Ok(None)` when it is not new
/// text, or why it cannot be read before the command runs.
pub(crate) fn message(
  args: &CommitArgs,
  dir: &Path,
  line: &Line,
) -> Result<Option<String>, String> {
  if args.origin != MessageOrigin::Written {
    return Ok(None);
  }
  let mut parts = Vec::new();
  for value in &args.messages {
    parts.push(value.clone().ok_or("its `-m` value expands at run time")?);
  }
  for file in &args.files {
    let text = match file.as_deref() {
      None => return Err("its `-F` path expands at run time".to_owned()),
      Some("-") => args.stdin.clone().ok_or("it is piped into `-F -`")?,
      Some(path) => read_file(dir, path, line)?,
    };
    parts.push(text);
  }
  if parts.is_empty() {
    return Err("it is written in an editor".to_owned());
  }
  let mut extra = Vec::new();
  for value in &args.trailers {
    let value = value
      .as_deref()
      .ok_or("a `--trailer` value expands at run time")?;
    extra.push(trailer_arg(value));
  }
  Ok(Some(with_trailers(&parts.join("\n\n"), &extra)))
}

#[cfg(test)]
#[path = "tests/message_test.rs"]
mod tests;
