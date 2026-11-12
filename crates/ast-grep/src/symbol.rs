//! `toolu ast-grep symbol <name> [<path>...]`: a name's definitions and its
//! references, each with the item that encloses it.
//!
//! `git grep` finds the hits; `toolu_engine::outline` turns the files with
//! hits into items, so a hit on an item's signature line is a definition and
//! every other hit names its innermost enclosing item.

mod render;

use std::path::Path;

use clap::ArgMatches;
use toolu_engine::outline::{FileOutline, enclosing, outline, supported};
use toolu_engine::quality::binary;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::process::{Spec, run as spawn};

/// Bytes of `git grep` output read.
const GREP_BYTES: usize = 256 * 1024;
/// Files with hits passed to one outline.
const MAX_OUTLINED: usize = 150;
/// The extensions `outline` reads, as the default pathspec.
const PATHSPEC: &[&str] = &[
  "*.rs", "*.ts", "*.mts", "*.cts", "*.tsx", "*.js", "*.mjs", "*.cjs", "*.jsx", "*.py", "*.pyi",
];

/// One `git grep` line: a path, a one-based line and the line's text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Hit {
  path: String,
  line: usize,
  text: String,
}

/// A hit on the signature line of an item with the looked-up name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Definition {
  path: String,
  line: usize,
  start: usize,
  end: usize,
  signature: String,
}

/// Any other hit, with its innermost enclosing item's name and range.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reference {
  path: String,
  line: usize,
  text: String,
  within: Option<(String, usize, usize)>,
}

/// Everything one lookup found, before the print caps.
#[derive(Debug, Default)]
struct Found {
  name: String,
  definitions: Vec<Definition>,
  references: Vec<Reference>,
  files: usize,
  /// Grep output passed [`GREP_BYTES`], so the counts are partial.
  partial: bool,
  /// Files with hits past [`MAX_OUTLINED`].
  unoutlined: usize,
  /// ast-grep is absent, so nothing was outlined.
  missing: bool,
}

/// The `git grep` command for `name` over `paths`, or the outlined languages.
fn grep_argv(name: &str, paths: &[String]) -> Vec<String> {
  let mut argv: Vec<String> = [
    "git",
    "grep",
    "-n",
    "-z",
    "-w",
    "-I",
    "-F",
    "--untracked",
    "-e",
    name,
    "--",
  ]
  .map(str::to_owned)
  .to_vec();
  if paths.is_empty() {
    argv.extend(PATHSPEC.iter().map(|spec| (*spec).to_owned()));
  } else {
    argv.extend(paths.iter().cloned());
  }
  argv
}

/// Run `git grep`; the hits and whether its output passed [`GREP_BYTES`].
fn grep(env: &Env, cwd: &Path, name: &str, paths: &[String]) -> Result<(Vec<Hit>, bool), String> {
  let mut spec = Spec::new(grep_argv(name, paths));
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  spec.max_output_bytes = GREP_BYTES;
  let output = spawn(&spec).map_err(|error| format!("git grep failed: {error:?}"))?;
  if output.timed_out {
    return Err("git grep timed out".to_owned());
  }
  // Exit 1 is git's "no match".
  if !output.truncated && !matches!(output.exit_code, 0 | 1) {
    let first = output.stderr.lines().next().unwrap_or_default();
    return Err(format!("git grep failed: {first}"));
  }
  Ok((
    parse(&output.stdout_bytes, output.truncated),
    output.truncated,
  ))
}

/// Parse `git grep -n -z` output: `path\0line\0text` per line. A `partial`
/// output's last line may be cut, so it is dropped.
fn parse(stdout: &[u8], partial: bool) -> Vec<Hit> {
  let text = String::from_utf8_lossy(stdout);
  let whole = if partial {
    text
      .rfind('\n')
      .map_or("", |end| text.get(..end).unwrap_or_default())
  } else {
    &text
  };
  whole
    .lines()
    .filter_map(|hit| {
      // `-z` ends the path and the line number with NUL, so a path may hold `:`.
      let mut parts = hit.splitn(3, '\0');
      let (path, line, text) = (parts.next()?, parts.next()?, parts.next()?);
      Some(Hit {
        path: path.to_owned(),
        line: line.parse().ok()?,
        text: text.trim().to_owned(),
      })
    })
    .collect()
}

/// Whether `text` is the first line of `signature`: one is a prefix of the
/// other once whitespace is collapsed and a `...` cap is dropped.
fn on_signature(text: &str, signature: &str) -> bool {
  let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
  let signature = signature.strip_suffix("...").unwrap_or(signature);
  let signature = signature.split_whitespace().collect::<Vec<_>>().join(" ");
  !line.is_empty()
    && line
      .chars()
      .zip(signature.chars())
      .all(|(left, right)| left == right)
}

/// Split `hits` into definitions and references against `outlines`.
fn classify(
  name: &str,
  hits: Vec<Hit>,
  outlines: &[FileOutline],
) -> (Vec<Definition>, Vec<Reference>) {
  let mut definitions = Vec::new();
  let mut references = Vec::new();
  for hit in hits {
    let file = outlines.iter().find(|file| file.path == hit.path);
    let own = file.and_then(|file| {
      file.items.iter().find(|item| {
        item.name == name
          && (item.start..=item.end).contains(&hit.line)
          && on_signature(&hit.text, &item.signature)
      })
    });
    if let Some(item) = own {
      definitions.push(Definition {
        path: hit.path,
        line: hit.line,
        start: item.start,
        end: item.end,
        signature: item.signature.clone(),
      });
      continue;
    }
    let within = file
      .and_then(|file| enclosing(file, hit.line))
      .map(|item| (item.name.clone(), item.start, item.end));
    references.push(Reference {
      path: hit.path,
      line: hit.line,
      text: hit.text,
      within,
    });
  }
  (definitions, references)
}

/// Look `name` up: grep, outline the files with hits, classify.
fn lookup(env: &Env, cwd: &Path, name: &str, paths: &[String]) -> Result<Found, String> {
  let (mut hits, partial) = grep(env, cwd, name, paths)?;
  hits.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
  let mut files: Vec<String> = hits.iter().map(|hit| hit.path.clone()).collect();
  files.dedup();
  let count = files.len();
  files.retain(|path| supported(path));
  let unoutlined = files.len().saturating_sub(MAX_OUTLINED);
  files.truncate(MAX_OUTLINED);
  let missing = binary(env, cwd).is_none();
  let outlines = if missing {
    Vec::new()
  } else {
    outline(env, cwd, &files)?
  };
  let (definitions, references) = classify(name, hits, &outlines);
  Ok(Found {
    name: name.to_owned(),
    definitions,
    references,
    files: count,
    partial,
    unoutlined,
    missing,
  })
}

/// Run `toolu ast-grep symbol`.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let name = matches
    .get_one::<String>("name")
    .cloned()
    .unwrap_or_default();
  if name.is_empty() {
    return Outcome::failed(
      Exit::Usage,
      "ast-grep symbol: name must not be empty".to_owned(),
    );
  }
  let paths: Vec<String> = matches
    .get_many::<String>("paths")
    .into_iter()
    .flatten()
    .cloned()
    .collect();
  let env = Env::process();
  let found = current_dir()
    .map_err(|error| error.to_string())
    .and_then(|cwd| lookup(&env, &cwd, &name, &paths));
  match found {
    Err(error) => Outcome::failed(Exit::Failure, format!("ast-grep symbol: {error}")),
    Ok(found) if found.files == 0 => Outcome::failed(
      Exit::Failure,
      format!("ast-grep symbol: no match for {name}"),
    ),
    Ok(found) => Outcome::data(render::render(&found, ctx.json)),
  }
}

#[cfg(test)]
#[path = "tests/symbol_test.rs"]
mod tests;
