//! `cargo xtask context-budget`: word ceilings for injected docs and skill descriptions.

use std::path::{Path, PathBuf};

use crate::options::Options;
use crate::{Verdict, output};

/// One reported line. The CLI prefixes it with `ok   ` or `RED  `.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Line {
  red: bool,
  text: String,
}

const FOLDED: &[&str] = &["", ">", ">-", "|", "|-"];

const DOC_BUDGETS: &[(&str, u32)] = &[
  ("session-start", 110),
  ("model-routing", 90),
  ("model-routing-opencode", 90),
  ("post-compaction", 28),
  ("session-start-ts", 30),
  ("session-start-rust", 36),
  ("session-start-python", 36),
];

struct Skill {
  name: &'static str,
  path: &'static str,
  budget: u32,
  phrases: &'static [&'static str],
}

const SKILLS: &[Skill] = &[
  Skill {
    name: "delivery-flow",
    path: "plugins/delivery-flow/skills/delivery-flow/SKILL.md",
    budget: 50,
    phrases: &["implement and deliver", "real-data execution", "PR"],
  },
  Skill {
    name: "brainstorm",
    path: "plugins/brainstorm/skills/brainstorm/SKILL.md",
    budget: 50,
    phrases: &["brainstorm", "trade-offs"],
  },
  Skill {
    name: "deep-research",
    path: "plugins/toolu/skills/deep-research/SKILL.md",
    budget: 80,
    phrases: &["deep research", "cited report"],
  },
  Skill {
    name: "toolu-review",
    path: "plugins/toolu-review/skills/review/SKILL.md",
    budget: 65,
    phrases: &["review before push"],
  },
  Skill {
    name: "ast-grep",
    path: "plugins/ast-grep/skills/ast-grep/SKILL.md",
    budget: 40,
    phrases: &[],
  },
  Skill {
    name: "jev",
    path: "plugins/jev/skills/jev/SKILL.md",
    budget: 60,
    phrases: &[],
  },
];

/// Which targets to check.
#[derive(Clone, Copy)]
pub(crate) enum Mode {
  Docs,
  Skills,
  All,
}

/// Run the check at `options.root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let mode = mode(&options.files)?;
  let lines = check(&options.root, mode);
  for line in &lines {
    let rendered = render(line);
    if line.red {
      output::error(&rendered);
    } else {
      output::say(&rendered);
    }
  }
  Ok(if lines.iter().any(|line| line.red) {
    Verdict::Findings
  } else {
    Verdict::Clean
  })
}

fn mode(files: &[PathBuf]) -> Result<Mode, String> {
  match files {
    [] => Ok(Mode::All),
    [word] => match word.to_str() {
      Some("docs") => Ok(Mode::Docs),
      Some("skills") => Ok(Mode::Skills),
      _ => Err("usage: cargo xtask context-budget [docs|skills]".to_owned()),
    },
    _ => Err("usage: cargo xtask context-budget [docs|skills]".to_owned()),
  }
}

fn render(line: &Line) -> String {
  if line.red {
    format!("RED  {}", line.text)
  } else {
    format!("ok   {}", line.text)
  }
}

pub(crate) fn check(root: &Path, mode: Mode) -> Vec<Line> {
  let mut lines = Vec::new();
  if matches!(mode, Mode::Docs | Mode::All) {
    lines.extend(DOC_BUDGETS.iter().flat_map(|(name, budget)| {
      check_doc(
        root,
        name,
        &format!("plugins/toolu/hooks/docs/{name}.md"),
        *budget,
      )
    }));
  }
  if matches!(mode, Mode::Skills | Mode::All) {
    lines.extend(SKILLS.iter().flat_map(|skill| check_skill(root, skill)));
  }
  lines
}

fn check_doc(root: &Path, name: &str, rel: &str, budget: u32) -> Vec<Line> {
  let Some(count) = word_count_file(&root.join(rel)) else {
    return vec![red(format!("{name}: MISSING {rel}"))];
  };
  vec![budget_line(name, "", count, budget, rel)]
}

fn check_skill(root: &Path, skill: &Skill) -> Vec<Line> {
  let Some(description) = extract_description(&root.join(skill.path)) else {
    return vec![red(format!("{}: MISSING {}", skill.name, skill.path))];
  };
  if description.is_empty() {
    return vec![red(format!(
      "{}: empty/unparseable description ({})",
      skill.name, skill.path
    ))];
  }
  let mut lines = vec![budget_line(
    skill.name,
    "desc ",
    word_count(&description),
    skill.budget,
    skill.path,
  )];
  for phrase in skill.phrases {
    if !phrase.is_empty() && !description.contains(phrase) {
      lines.push(red(format!(
        "{}: missing trigger phrase \"{phrase}\" ({})",
        skill.name, skill.path
      )));
    }
  }
  lines
}

fn budget_line(name: &str, kind: &str, count: u32, budget: u32, rel: &str) -> Line {
  if count <= budget {
    Line {
      red: false,
      text: format!("{name} {kind}{count}w (<= {budget})"),
    }
  } else {
    red(format!("{name} {kind}{count}w EXCEEDS {budget} ({rel})"))
  }
}

fn red(text: String) -> Line {
  Line { red: true, text }
}

/// Word count of a regular file, or `None` when it is absent.
fn word_count_file(path: &Path) -> Option<u32> {
  read_regular(path).map(|text| word_count(&text))
}

fn word_count(text: &str) -> u32 {
  u32::try_from(text.split_whitespace().count()).unwrap_or(u32::MAX)
}

fn read_regular(path: &Path) -> Option<String> {
  let meta = std::fs::metadata(path).ok()?;
  if !meta.is_file() {
    return None;
  }
  std::fs::read_to_string(path).ok()
}

/// The YAML `description`, or `None` when the file is absent.
fn extract_description(path: &Path) -> Option<String> {
  let text = read_regular(path)?;
  let mut folded = false;
  let mut buf = String::new();
  for line in lines_of(&text) {
    if !folded {
      match description_start(line) {
        Start::Skip => continue,
        Start::Text(value) => return Some(value),
        Start::Fold => folded = true,
      }
      continue;
    }
    if line == "---" || is_key(line) {
      break;
    }
    push_part(&mut buf, line.trim_start_matches([' ', '\t']));
  }
  Some(if folded { buf } else { String::new() })
}

enum Start {
  Skip,
  Text(String),
  Fold,
}

fn description_start(line: &str) -> Start {
  let Some(value) = description_value(line) else {
    return Start::Skip;
  };
  if FOLDED.contains(&value) {
    Start::Fold
  } else {
    Start::Text(value.to_owned())
  }
}

fn description_value(line: &str) -> Option<&str> {
  line
    .strip_prefix("description:")
    .map(|value| value.trim_start_matches([' ', '\t']))
}

fn push_part(buf: &mut String, part: &str) {
  if !buf.is_empty() {
    buf.push(' ');
  }
  buf.push_str(part);
}

fn lines_of(text: &str) -> Vec<&str> {
  let mut lines: Vec<&str> = text.split('\n').collect();
  if lines.last().is_some_and(|line| line.is_empty()) {
    lines.pop();
  }
  lines
}

fn is_key(line: &str) -> bool {
  let Some((key, _)) = line.split_once(':') else {
    return false;
  };
  !key.is_empty()
    && key
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

#[cfg(test)]
#[path = "tests/context_budget_test.rs"]
mod tests;
