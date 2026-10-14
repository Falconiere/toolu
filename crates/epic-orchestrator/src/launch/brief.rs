//! The worker brief and the prompts that point the agent at it
//! (`renderBrief`, `START_PROMPT`, `RESUME_PROMPT`).

use std::path::Path;

use regex::Regex;

use crate::graph::{Graph, GraphIssue};
use crate::hosts::{HostKind, skill_ref};

const TEMPLATE: &str = include_str!(
  "../../../../plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md"
);

const START_PROMPT: &str = "You are an epic worker. Read {brief} and follow it exactly, starting at Pipeline step 1. Report status with the command it gives.";
const RESUME_PROMPT: &str = "You are an epic worker resuming interrupted work. Read {brief} and follow it. First inspect {status}, `git log origin/{base}..HEAD`, `git status`, and any open PR for this branch, then continue from the last reported phase instead of starting over.";

/// Where one issue's worker works and reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BriefPaths {
  /// The worktree.
  pub worktree: String,
  /// `status/<key>.json`.
  pub status: String,
  /// `briefs/<key>.md`.
  pub brief: String,
}

impl BriefPaths {
  /// The status and brief paths of `key` under `state`.
  pub(crate) fn of(state: &Path, key: &str, worktree: &str) -> Self {
    Self {
      worktree: worktree.to_owned(),
      status: state
        .join("status")
        .join(format!("{key}.json"))
        .display()
        .to_string(),
      brief: state
        .join("briefs")
        .join(format!("{key}.md"))
        .display()
        .to_string(),
    }
  }
}

/// How the worker reads its issue and what its PR body must say.
fn tracker_lines(graph: &Graph, issue: &GraphIssue) -> (String, String) {
  match graph.tracker.as_deref() {
    Some("jira") => (
      format!("`toolu epic jira issue get {}`", issue.reference),
      format!("Resolves {}", issue.reference),
    ),
    Some("linear") => (
      format!("the Linear issue {}", issue.url),
      format!("Fixes {}", issue.reference),
    ),
    _ => (
      format!("`gh issue view {} --comments`", issue.url),
      format!("Closes {}", issue.reference),
    ),
  }
}

/// The worker a brief is written for: its host, and whether its approval
/// prompts are on (`--safe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BriefHost {
  /// The host.
  pub kind: HostKind,
  /// Approval prompts are on.
  pub safe: bool,
}

/// What the brief says about approvals and questions in the pane.
fn approval_lines(safe: bool) -> (&'static str, &'static str) {
  if safe {
    (
      "Approval prompts are on and a human may refuse a tool call; keep these limits either way.",
      "A human may answer approval prompts in this pane, but nobody answers questions.",
    )
  } else {
    (
      "You run without approval prompts, so these limits are yours to keep.",
      "Nobody answers questions in this pane.",
    )
  }
}

/// The issue's closed blockers, comma-separated, or `none`.
fn closed_blockers(issue: &GraphIssue) -> String {
  let done: Vec<&str> = issue
    .blockers
    .iter()
    .filter(|(_, state)| *state == "closed")
    .map(|(blocker, _)| blocker.as_str())
    .collect();
  if done.is_empty() {
    "none".to_owned()
  } else {
    done.join(", ")
  }
}

/// The brief of `issue` for a worker on `host`.
///
/// # Errors
/// The brief template comment pattern fails to compile.
pub(crate) fn render_brief(
  graph: &Graph,
  issue: &GraphIssue,
  paths: &BriefPaths,
  base: &str,
  host: BriefHost,
) -> Result<String, String> {
  let kind = host.kind;
  let (approvals, questions) = approval_lines(host.safe);
  let (read, closes) = tracker_lines(graph, issue);
  let values = [
    ("ISSUE_REF", issue.reference.clone()),
    ("ISSUE_URL", issue.url.clone()),
    ("ISSUE_TITLE", issue.title.clone()),
    ("ISSUE_READ", read),
    ("CLOSES", closes),
    ("HOST", kind.name().to_owned()),
    (
      "DELIVERY",
      skill_ref(kind, "delivery-flow", "delivery-flow"),
    ),
    ("BABYSIT", skill_ref(kind, "pr-babysit", "babysit")),
    ("DEBUG", skill_ref(kind, "toolu", "debug")),
    ("EPIC_REF", graph.epic.reference.clone()),
    ("EPIC_URL", graph.epic.url.clone()),
    ("EPIC_TITLE", graph.epic.title.clone()),
    ("WORKTREE", paths.worktree.clone()),
    ("BRANCH", issue.branch.clone()),
    ("BASE", base.to_owned()),
    ("STATUS_FILE", paths.status.clone()),
    ("BLOCKERS", closed_blockers(issue)),
    ("APPROVALS", approvals.to_owned()),
    ("QUESTIONS", questions.to_owned()),
  ];
  let comment = Regex::new(r"^<!--[\s\S]*?-->\n+").map_err(|err| err.to_string())?;
  let mut text = comment.replace(TEMPLATE, "").into_owned();
  for (key, value) in values {
    text = text.replace(&format!("{{{{{key}}}}}"), &value);
  }
  Ok(text)
}

/// The start or resume prompt for `paths`.
pub(crate) fn prompt_text(paths: &BriefPaths, base: &str, resuming: bool, resumed: bool) -> String {
  let template = if resuming {
    RESUME_PROMPT
  } else {
    START_PROMPT
  };
  let mut text = template
    .replacen("{brief}", &paths.brief, 1)
    .replacen("{status}", &paths.status, 1)
    .replacen("{base}", base, 1);
  if resumed {
    text.push_str(" Your previous conversation for this issue is loaded above.");
  }
  text
}

#[cfg(test)]
#[path = "tests/brief_test.rs"]
mod tests;
