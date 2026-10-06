//! Which Markdown files are scanned, and the code in them: shell-tagged
//! fenced blocks and single-line inline code spans outside fences.

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
  block: Option<Block>,
}

/// Split `markdown` into shell blocks and inline spans. An unterminated fence
/// runs to the end of the file.
pub(crate) fn scan(markdown: &str) -> Scanned {
  let mut out = Scanned::default();
  let mut fence: Option<Fence> = None;
  for (index, raw) in markdown.lines().enumerate() {
    let line = index + 1;
    let trimmed = raw.trim_start();
    match fence.as_mut() {
      Some(open) if closes(trimmed, open.marker, open.len) => {
        out.blocks.extend(fence.take().and_then(|open| open.block));
      }
      Some(open) => {
        if let Some(block) = open.block.as_mut() {
          block.text.push_str(&content(trimmed, raw, open.console));
          block.text.push('\n');
        }
      }
      None => match opens(trimmed) {
        Some((marker, len, info)) => fence = Some(open_fence(marker, len, &info, line)),
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
    block: shell.then(|| Block {
      line: line + 1,
      text: String::new(),
    }),
  }
}

/// A content line; outside a `console` prompt line it becomes empty, so line
/// numbers stay aligned.
fn content(trimmed: &str, raw: &str, console: bool) -> String {
  if !console {
    return raw.to_owned();
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

/// Directory entry names, sorted; an absent directory has none.
pub(crate) fn entries(dir: &Path) -> Result<Vec<String>, String> {
  let Ok(read) = std::fs::read_dir(dir) else {
    return Ok(Vec::new());
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
