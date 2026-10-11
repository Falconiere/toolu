//! The byte-compatible Claude line, including ANSI colour and segment order.

use std::fmt::Write as _;

use serde_json::json;
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::host::roots::Roots;

use crate::collect::{Jev, Project, collect};
use crate::payload::{Payload, read};

const CYAN: &str = "\u{1b}[36m";
const GREEN: &str = "\u{1b}[32m";
const YELLOW: &str = "\u{1b}[33m";
const MAGENTA: &str = "\u{1b}[35m";
const BLUE: &str = "\u{1b}[34m";
const RED: &str = "\u{1b}[31m";
const DIM: &str = "\u{1b}[2m";
const BOLD: &str = "\u{1b}[1m";
const RESET: &str = "\u{1b}[0m";
const SEP: &str = "\u{1b}[2m | \u{1b}[0m";

fn add(line: &mut String, segment: &str) {
  if !segment.is_empty() {
    line.push_str(SEP);
    line.push_str(segment);
  }
}

fn tokens(text: &str) -> String {
  if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
    return "0".to_owned();
  }
  let Ok(n) = text.parse::<u128>() else {
    return "0".to_owned();
  };
  if n >= 1_000_000 {
    format!("{}.{}M", n / 1_000_000, n % 1_000_000 / 100_000)
  } else if n >= 1000 {
    format!("{}k", n / 1000)
  } else {
    n.to_string()
  }
}

fn percentage(text: &str) -> Option<String> {
  let mut parts = text.split('.');
  let whole = parts.next()?;
  let decimal = parts.next();
  if whole.is_empty()
    || !whole.bytes().all(|byte| byte.is_ascii_digit())
    || decimal.is_some_and(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
    || parts.next().is_some()
  {
    return None;
  }
  let number = text.parse::<f64>().ok()?;
  Some(format!("{:.0}", number.round_ties_even()))
}

fn context(payload: &Payload) -> String {
  let value = format!(
    "{}/{}",
    tokens(&payload.ctx_used),
    tokens(&payload.ctx_size)
  );
  match percentage(&payload.ctx_pct) {
    Some(pct) => format!("{value} ({pct}%)"),
    None => value,
  }
}

fn tree_suffix(project: &Project) -> String {
  let mut out = String::new();
  if project.ahead > 0 || project.behind > 0 {
    out.push_str(DIM);
    if project.ahead > 0 {
      let _written = write!(out, "↑{}", project.ahead);
    }
    if project.behind > 0 {
      let _written = write!(out, "↓{}", project.behind);
    }
    out.push_str(RESET);
  }
  let mut dirty = Vec::new();
  if project.staged > 0 {
    dirty.push(format!("+{}", project.staged));
  }
  if project.unstaged > 0 {
    dirty.push(format!("~{}", project.unstaged));
  }
  if project.untracked > 0 {
    dirty.push(format!("?{}", project.untracked));
  }
  if !dirty.is_empty() {
    let _written = write!(out, "{YELLOW}[{}]{RESET}", dirty.join(" "));
  }
  out
}

/// The line's exact bytes, without a terminating newline.
pub(crate) fn line(payload: &Payload, project: &Project) -> String {
  let mut out = format!("{CYAN}{}{RESET}", payload.model);
  if !payload.effort.is_empty() && payload.effort != "null" {
    add(
      &mut out,
      &format!("{YELLOW}effort:{}{RESET}", payload.effort),
    );
  }
  add(
    &mut out,
    &format!("{MAGENTA}ctx:{}{RESET}", context(payload)),
  );
  if !project.account_domain.is_empty() {
    add(
      &mut out,
      &format!("{GREEN}{}{RESET}", project.account_domain),
    );
  }
  if project.gate_failing {
    add(&mut out, &format!("{BOLD}{RED}✗ gate:failing{RESET}"));
  }
  if !project.folder.is_empty() {
    add(&mut out, &format!("{BOLD}{}{RESET}", project.folder));
  }
  if !project.branch.is_empty() {
    add(&mut out, &format!("{BLUE}{}{RESET}", project.branch));
  }
  if !project.repo_root.is_empty() {
    out.push_str(&tree_suffix(project));
  }
  if let Some(count) = &project.comemory_count {
    add(&mut out, &format!("{BOLD}{GREEN}[COMEMORY:{count}]{RESET}"));
  }
  match &project.jev {
    Jev::Ready => add(&mut out, &format!("{BOLD}{GREEN}[JEV:READY]{RESET}")),
    Jev::Unavailable(reason) => {
      add(
        &mut out,
        &format!("{BOLD}{YELLOW}[JEV:UNAVAILABLE: {reason}]{RESET}"),
      );
    }
    Jev::Hidden => {}
  }
  out
}

/// Render the payload through the injected real status snapshot.
pub(crate) fn run(ctx: &Ctx, roots: &Roots, status: &dyn StatusSnapshot) -> Outcome {
  let payload = read(ctx.stdin.as_deref());
  let project = collect(&payload, roots, status);
  let value = line(&payload, &project);
  if ctx.json {
    Outcome::data(json!({ "line": value }).to_string())
  } else {
    Outcome::data(value)
  }
}

#[cfg(test)]
#[path = "tests/render_test.rs"]
mod tests;
