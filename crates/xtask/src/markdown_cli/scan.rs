//! Which Markdown files are scanned, and the code in them: shell-tagged
//! fenced blocks and single-line inline code spans outside fences.

use std::io::ErrorKind;
use std::path::Path;

/// Fence info strings whose blocks are shell.
const SHELL: &[&str] = &["bash", "sh", "shell", "zsh", "fish", "console"];

/// Directories under `docs/` that hold dated records, not instructions.
const RECORDS: &[&str] = &["docs/toolu", "docs/releases"];

/// A shell-tagged fenced block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Block {
  /// The Markdown line of the block's first content line.
  pub(crate) line: usize,
  /// The content; a `console` block keeps only its `$ ` lines, prompt dropped.
  pub(crate) text: String,
}

/// An inline code span.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Span {
  /// Its Markdown line.
  pub(crate) line: usize,
  /// Its text, one leading and trailing space trimmed (`CommonMark`).
  pub(crate) text: String,
}

/// The code a Markdown file holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Scanned {
  /// Shell-tagged fenced blocks.
  pub(crate) blocks: Vec<Block>,
  /// Inline code spans outside fences.
  pub(crate) spans: Vec<Span>,
}

/// An open fence: its character, length, whether it is shell, and the block.
struct Fence {
  marker: char,
  len: usize,
  console: bool,
  /// Opened inside a blockquote: its lines carry `>` markers.
  quoted: bool,
  block: Option<Block>,
}

/// Split `markdown` into shell blocks and inline spans. An unterminated fence
/// runs to the end of the file.
pub(crate) fn scan(markdown: &str) -> Scanned {
  let mut out = Scanned::default();
  let mut fence: Option<Fence> = None;
  for (index, raw) in markdown.lines().enumerate() {
    let line = index + 1;
    let plain = raw.trim_start();
    let unquoted = unblockquote(raw);
    let text = |quoted: bool| if quoted { unquoted } else { plain };
    match fence.as_mut() {
      Some(open) if closes(text(open.quoted), open.marker, open.len) => {
        out.blocks.extend(fence.take().and_then(|open| open.block));
      }
      Some(open) => {
        if let Some(block) = open.block.as_mut() {
          block
            .text
            .push_str(&content(text(open.quoted), open.console));
          block.text.push('\n');
        }
      }
      None => match opens(unquoted) {
        Some((marker, len, info)) => {
          let mut opened = open_fence(marker, len, &info, line);
          opened.quoted = plain.starts_with('>');
          fence = Some(opened);
        }
        None => out.spans.extend(spans(raw, line)),
      },
    }
  }
  out.blocks.extend(fence.and_then(|open| open.block));
  out
}

fn open_fence(marker: char, len: usize, info: &str, line: usize) -> Fence {
  let tag = info
    .split_whitespace()
    .next()
    .unwrap_or_default()
    .to_lowercase();
  let shell = SHELL.contains(&tag.as_str());
  Fence {
    marker,
    len,
    console: tag == "console",
    quoted: false,
    block: shell.then(|| Block {
      line: line + 1,
      text: String::new(),
    }),
  }
}

/// The line without its indentation and blockquote markers (`> `), so a
/// fence quoted in a callout is still read.
fn unblockquote(raw: &str) -> &str {
  let mut line = raw.trim_start();
  while let Some(rest) = line.strip_prefix('>') {
    line = rest.trim_start();
  }
  line
}

/// A content line; outside a `console` prompt line it becomes empty, so line
/// numbers stay aligned.
fn content(trimmed: &str, console: bool) -> String {
  if !console {
    return trimmed.to_owned();
  }
  trimmed.strip_prefix("$ ").unwrap_or_default().to_owned()
}

/// ```` ``` ```` or `~~~` (three or more) and the info string after it.
fn opens(trimmed: &str) -> Option<(char, usize, String)> {
  let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
  let len = trimmed.chars().take_while(|c| *c == marker).count();
  let info: String = trimmed.chars().skip(len).collect();
  (len >= 3 && !(marker == '`' && info.contains('`')))
    .then(|| (marker, len, info.trim().to_owned()))
}

fn closes(trimmed: &str, marker: char, len: usize) -> bool {
  let run = trimmed.chars().take_while(|c| *c == marker).count();
  run >= len && trimmed.chars().skip(run).all(char::is_whitespace)
}

/// The inline code spans of one line: a run of N backticks up to the next run
/// of exactly N.
fn spans(raw: &str, line: usize) -> Vec<Span> {
  let chars: Vec<char> = raw.chars().collect();
  let mut found = Vec::new();
  let mut pos = 0;
  while let Some(start) = chars
    .iter()
    .skip(pos)
    .position(|c| *c == '`')
    .map(|at| at + pos)
  {
    let len = run(&chars, start);
    let body = start + len;
    let Some(end) = closing(&chars, body, len) else {
      pos = body;
      continue;
    };
    let text: String = chars.iter().skip(body).take(end - body).collect();
    found.push(Span {
      line,
      text: trim_one(&text),
    });
    pos = end + len;
  }
  found
}

fn run(chars: &[char], from: usize) -> usize {
  chars.iter().skip(from).take_while(|c| **c == '`').count()
}

fn closing(chars: &[char], from: usize, len: usize) -> Option<usize> {
  let mut pos = from;
  while let Some(at) = chars
    .iter()
    .skip(pos)
    .position(|c| *c == '`')
    .map(|at| at + pos)
  {
    let found = run(chars, at);
    if found == len {
      return Some(at);
    }
    pos = at + found;
  }
  None
}

fn trim_one(text: &str) -> String {
  match text
    .strip_prefix(' ')
    .and_then(|inner| inner.strip_suffix(' '))
  {
    Some(inner) if !inner.trim().is_empty() => inner.to_owned(),
    _ => text.to_owned(),
  }
}

/// The scanned files under `root`, repository-relative with `/`, sorted:
/// `plugins/*/skills/**`, `plugins/*/commands/*`, `plugins/*/agents/*`,
/// `AGENTS.md` and `docs/**` without the dated records.
pub(crate) fn files(root: &Path) -> Result<Vec<String>, String> {
  let mut found = Vec::new();
  for plugin in entries(&root.join("plugins"))? {
    let name = format!("plugins/{plugin}");
    walk(root, &format!("{name}/skills"), true, &mut found)?;
    walk(root, &format!("{name}/commands"), false, &mut found)?;
    walk(root, &format!("{name}/agents"), false, &mut found)?;
  }
  if root.join("AGENTS.md").is_file() {
    found.push("AGENTS.md".to_owned());
  }
  walk(root, "docs", true, &mut found)?;
  found.retain(|file| {
    !RECORDS
      .iter()
      .any(|dir| file.starts_with(&format!("{dir}/")))
  });
  found.sort();
  Ok(found)
}

/// Directory entry names, sorted; an absent directory, or a file where a
/// directory may be, has none. Any other error is a setup error.
pub(crate) fn entries(dir: &Path) -> Result<Vec<String>, String> {
  let read = match std::fs::read_dir(dir) {
    Ok(read) => read,
    Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
      return Ok(Vec::new());
    }
    Err(err) => return Err(format!("cannot list {}: {err}", dir.display())),
  };
  let mut names = Vec::new();
  for entry in read {
    let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
    names.push(entry.file_name().to_string_lossy().into_owned());
  }
  names.sort();
  Ok(names)
}

fn walk(root: &Path, dir: &str, deep: bool, found: &mut Vec<String>) -> Result<(), String> {
  for name in entries(&root.join(dir))? {
    let path = format!("{dir}/{name}");
    let full = root.join(&path);
    if full.is_dir() {
      if deep {
        walk(root, &path, deep, found)?;
      }
    } else if Path::new(&path)
      .extension()
      .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
      found.push(path);
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/scan_test.rs"]
mod tests;
