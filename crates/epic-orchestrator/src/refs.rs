//! Issue references and the names derived from them (`scripts/common.ts`):
//! `owner/repo#N` parsing, herdr agent keys, branch slugs and the epic state
//! home of each host.

use std::path::PathBuf;
use std::time::Duration;

use regex::Regex;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

/// One GitHub issue or pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueRef {
  /// Repository owner.
  pub owner: String,
  /// Repository name.
  pub repo: String,
  /// Issue or pull request number.
  pub number: u64,
}

impl IssueRef {
  /// The canonical `owner/repo#N`.
  pub(crate) fn canonical(&self) -> String {
    format!("{}/{}#{}", self.owner, self.repo, self.number)
  }
}

const URL: &str = r"github\.com/([^/\s]+)/([^/\s]+)/(?:issues|pull)/(\d+)";
const SHORT: &str = r"^([\w.-]+)/([\w.-]+)#(\d+)$";
/// URL, `owner/repo#N`, or `#N` (a bare `#N` after a word or `/` is not a ref).
const IN_TEXT: &str = r"https://github\.com/[^\s)]+/issues/\d+|[\w.-]+/[\w.-]+#\d+|#\d+";

fn compiled(pattern: &str) -> Result<Regex, String> {
  Regex::new(pattern).map_err(|err| err.to_string())
}

/// Parse a URL, `owner/repo#N`, or `#N`/`N` against `default_repo`. Without a
/// default, a bare number asks `gh repo view` for the current repository.
///
/// # Errors
/// The text is no reference, or the default repository is malformed.
pub(crate) fn parse_ref(text: &str, default_repo: Option<&str>) -> Result<IssueRef, String> {
  let trimmed = text.trim();
  if let Some(found) = compiled(URL)?.captures(trimmed)
    && let Some(parsed) = from_captures(&found)
  {
    return Ok(parsed);
  }
  if let Some(found) = compiled(SHORT)?.captures(trimmed)
    && let Some(parsed) = from_captures(&found)
  {
    return Ok(parsed);
  }
  let bare = trimmed.strip_prefix('#').unwrap_or(trimmed);
  if !bare.is_empty() && bare.bytes().all(|byte| byte.is_ascii_digit()) {
    let repo = match default_repo {
      Some(repo) => repo.to_owned(),
      None => current_repo()?,
    };
    let Some((owner, name)) = repo.split_once('/').filter(|(owner, _)| !owner.is_empty()) else {
      return Err(format!("bad default_repo: {repo}"));
    };
    let number = bare.parse::<u64>().map_err(|err| err.to_string())?;
    return Ok(IssueRef {
      owner: owner.to_owned(),
      repo: name.to_owned(),
      number,
    });
  }
  Err(format!("unrecognized issue reference: {}", json_text(text)))
}

fn from_captures(found: &regex::Captures<'_>) -> Option<IssueRef> {
  Some(IssueRef {
    owner: found.get(1)?.as_str().to_owned(),
    repo: found.get(2)?.as_str().to_owned(),
    number: found.get(3)?.as_str().parse().ok()?,
  })
}

fn json_text(text: &str) -> String {
  serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""))
}

/// Every reference in `text`, canonical, against `default_repo`.
///
/// # Errors
/// The pattern cannot compile.
pub(crate) fn refs_in(text: &str, default_repo: &str) -> Result<Vec<String>, String> {
  let mut out = Vec::new();
  for found in compiled(IN_TEXT)?.find_iter(text) {
    let word = found.as_str();
    if word.starts_with('#') && preceded_by_name(text, found.start()) {
      continue;
    }
    if let Ok(parsed) = parse_ref(word, Some(default_repo)) {
      out.push(parsed.canonical());
    }
  }
  Ok(out)
}

fn preceded_by_name(text: &str, start: usize) -> bool {
  text
    .get(..start)
    .and_then(|head| head.chars().next_back())
    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '/')
}

/// `gh repo view`'s `nameWithOwner` for the working directory.
///
/// # Errors
/// `gh` is missing, fails, or times out.
pub(crate) fn current_repo() -> Result<String, String> {
  let mut spec = Spec::new([
    "gh",
    "repo",
    "view",
    "--json",
    "nameWithOwner",
    "-q",
    ".nameWithOwner",
  ]);
  spec.timeout = Duration::from_secs(30);
  let output = run(&spec).map_err(|err| format!("gh repo view failed: {err:?}"))?;
  if output.exit_code != 0 || output.timed_out {
    return Err(format!("gh repo view failed: {}", output.stderr.trim()));
  }
  Ok(output.stdout.trim().to_owned())
}

/// A herdr agent name, `[a-z][a-z0-9_-]{0,31}`: the folded `owner/repo` plus `-N`.
pub(crate) fn issue_key(repo: &str, number: u64) -> String {
  let mut base = fold(repo).trim_matches('-').to_owned();
  if base.is_empty() {
    "repo".clone_into(&mut base);
  }
  if !base.starts_with(|ch: char| ch.is_ascii_lowercase()) {
    base.insert(0, 'r');
  }
  let suffix = format!("-{number}");
  let keep = 32_usize.saturating_sub(suffix.len());
  let head: String = base.chars().take(keep).collect();
  format!("{}{suffix}", head.trim_end_matches('-'))
}

/// Lowercase, with every run outside `[a-z0-9_-]` replaced by one `-`.
fn fold(text: &str) -> String {
  let mut out = String::new();
  let mut gap = false;
  for ch in text.to_lowercase().chars() {
    if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-' {
      out.push(ch);
      gap = false;
    } else if !gap {
      out.push('-');
      gap = true;
    }
  }
  out
}

/// The first `words` lowercase words of a title without its `[Prefix]`.
pub(crate) fn slugify(title: &str, words: usize) -> String {
  let cleaned = strip_prefix(title);
  let parts: Vec<String> = cleaned
    .to_lowercase()
    .split(|ch: char| !(ch.is_ascii_lowercase() || ch.is_ascii_digit()))
    .filter(|part| !part.is_empty())
    .take(words)
    .map(str::to_owned)
    .collect();
  if parts.is_empty() {
    "work".to_owned()
  } else {
    parts.join("-")
  }
}

/// `title` without one leading `[...]` group and the space around it.
pub(crate) fn strip_prefix(title: &str) -> &str {
  let start = title.trim_start();
  if let Some(rest) = start.strip_prefix('[')
    && let Some((_, after)) = rest.split_once(']')
  {
    return after.trim_start();
  }
  title
}

/// The herdr key of a work item: `issue_key` for GitHub, the folded key otherwise.
pub(crate) fn key_for(reference: &str, repo: &str, number: Option<u64>) -> String {
  match number {
    Some(number) => issue_key(repo, number),
    None => fold(reference).chars().take(32).collect(),
  }
}

/// `feat/<number or key>-<slug>`.
pub(crate) fn branch_for(reference: &str, number: Option<u64>, title: &str) -> String {
  let id = number.map_or_else(|| reference.to_lowercase(), |number| number.to_string());
  format!("feat/{id}-{}", slugify(title, 5))
}

/// Where epic state lives: `EPIC_STATE_HOME`; else `$CODEX_HOME/toolu/epics`
/// (default `~/.codex`) when `TOOLU_HOST_OVERRIDE=codex`, or when `CODEX_HOME`
/// is set without `CLAUDE_CONFIG_DIR`; else `~/.claude/epics`. That last
/// default is every orchestrator's, Codex's included: no host's shell tool
/// exports `CODEX_HOME` or the override.
pub(crate) fn epics_home(env: &Env) -> PathBuf {
  if let Some(home) = env.get("EPIC_STATE_HOME") {
    return PathBuf::from(home);
  }
  let host = env.get("TOOLU_HOST_OVERRIDE").unwrap_or("").to_lowercase();
  let unset = host.is_empty();
  let codex = env.get("CODEX_HOME");
  if host == "codex" || (unset && codex.is_some() && env.get("CLAUDE_CONFIG_DIR").is_none()) {
    let root = codex.map_or_else(|| env.home().join(".codex"), PathBuf::from);
    return root.join("toolu").join("epics");
  }
  env.home().join(".claude").join("epics")
}

#[cfg(test)]
#[path = "tests/refs_test.rs"]
mod tests;
