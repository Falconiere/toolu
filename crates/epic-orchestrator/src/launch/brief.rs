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

/// The worker a brief is written for: its host, whether its approval
/// prompts are on (`--safe`), and whether the epic merges on the local gate
/// without pr-babysit (`--no-babysit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BriefHost {
  /// The host.
  pub kind: HostKind,
  /// Approval prompts are on.
  pub safe: bool,
  /// No CI: the engine never runs babysit and merges on the local gate.
  pub no_babysit: bool,
}

impl BriefHost {
  /// The brief host of a launch in the epic at `state_dir`, whose
  /// `launch.json` says whether babysit is off.
  pub(crate) fn at(kind: HostKind, safe: bool, state_dir: &Path) -> Self {
    Self {
      kind,
      safe,
      no_babysit: crate::epics::launch_settings(state_dir).no_babysit,
    }
  }
}

const WATCH: &str =
  "The engine checks your PR every 3\n   minutes and prompts you only when something needs you.";
const PROMPTED: &str = "5. **When prompted.** A tick result starts with
   `pr-babysit: <repo>#<n> needs you (...)` and lists the items. Act on them
   with the `toolu babysit` commands it names (reply, resolve, record; the
   {{BABYSIT}} skill's references explain each), fix and push, run
   `report babysit --pr <number>` again, and stop. When CI is green, every
   thread is resolved and the bot verdict is clean, the engine marks the issue
   `ready`, queues it and merges it; you report nothing. Your session stays
   idle until the issue is merged and cleaned up.
";
const TICK_MESSAGE: &str = "- **A tick result** — see Pipeline step 5.\n";
/// Under `--no-babysit`: what the engine merges on instead of CI.
const LOCAL_WATCH: &str = "This epic runs without CI: the engine
   merges your PR itself once your plan ledger is fresh-green on the pushed
   head (the full gate included), the PR contains `origin/{{BASE}}` and every
   review thread is resolved. It never prompts you about checks; you report
   nothing more, and your session stays idle until the issue is merged and
   cleaned up.";

/// The babysit lines of the brief: the authorization, what follows the
/// babysit report, step 5 and its message line.
fn babysit_lines(host: BriefHost) -> [(&'static str, String); 5] {
  if host.no_babysit {
    [
      ("PR_AUTH", ", and open the PR".to_owned()),
      ("WATCH", LOCAL_WATCH.to_owned()),
      ("PROMPTED", String::new()),
      ("TICK_MESSAGE", String::new()),
      ("BABYSIT", "babysit".to_owned()),
    ]
  } else {
    let skill = skill_ref(host.kind, "pr-babysit", "babysit");
    // Step 5 names the skill itself, so it lands before the skill fills in.
    [
      ("PR_AUTH", format!(", open the PR, and run {skill}")),
      ("WATCH", WATCH.to_owned()),
      ("PROMPTED", PROMPTED.to_owned()),
      ("TICK_MESSAGE", TICK_MESSAGE.to_owned()),
      ("BABYSIT", skill),
    ]
  }
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
  let values = babysit_lines(host).into_iter().chain([
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
  ]);
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
