use std::path::Path;

use super::{BriefHost, BriefPaths, prompt_text, render_brief};
use crate::graph::Graph;
use crate::hosts::{HostKind, skill_ref};
use crate::support::graph_248;

fn paths() -> BriefPaths {
  BriefPaths::of(Path::new("/state"), "comemory-255", "/wt/comemory/feat-255")
}

fn brief(graph: &Graph, wanted: &str, kind: HostKind) -> String {
  let issue = graph.find(wanted).expect("issue");
  let host = BriefHost { kind, safe: false };
  render_brief(graph, issue, &paths(), "main", host).expect("brief")
}

#[test]
fn every_placeholder_is_filled_with_toolu_epic_commands() {
  let graph = graph_248();
  let text = brief(&graph, "Falconiere/comemory#255", HostKind::Claude);
  assert!(!text.contains("{{") && !text.contains("<!--"));
  assert!(text.starts_with("# Epic worker brief — Falconiere/comemory#255"));
  assert!(
    text.contains("Closes Falconiere/comemory#255")
      && text.contains("Part of Falconiere/comemory#248")
  );
  assert!(
    text.contains("`gh issue view https://github.com/Falconiere/comemory/issues/255 --comments`")
  );
  assert!(text.contains("| `toolu epic report <phase> --status-file /state/status/comemory-255.json`, adding `--pr N` or `--note \"...\"` where a step asks |"));
  assert!(text.contains("`toolu epic job -- <command> <args...>`"));
  for phrase in [
    "one blocking call",
    "never sleep and\nretry",
    "write_stdin",
    "full log",
  ] {
    assert!(text.contains(phrase), "{phrase}");
  }
  assert!(!text.contains("wait and retry"));
  assert!(!text.contains("bun") && !text.contains("scripts/"));
  for phase in [
    "report brainstorm",
    "report spec-review",
    "report execution",
    "/delivery-flow:delivery-flow",
    "/pr-babysit:babysit",
  ] {
    assert!(text.contains(phase), "{phase}");
  }
}

#[test]
fn jira_and_linear_items_read_through_their_own_lines_on_every_host() {
  let mut graph = graph_248();
  graph.tracker = Some("jira".into());
  if let Some(issue) = graph
    .issues
    .iter_mut()
    .find(|issue| issue.key == "comemory-255")
  {
    issue.reference = "PAY-12".into();
    issue.number = None;
  }
  for kind in crate::hosts::HOST_KINDS {
    let text = brief(&graph, "PAY-12", kind);
    assert!(
      text.contains("Resolves PAY-12") && text.contains("`toolu epic jira issue get PAY-12`")
    );
    assert!(!text.contains("{{"), "{kind:?}");
  }
  graph.tracker = Some("linear".into());
  let linear = brief(&graph, "PAY-12", HostKind::Codex);
  assert!(
    linear.contains("Fixes PAY-12") && linear.contains("the Linear issue https://github.com/")
  );
}

#[test]
fn prompts_point_at_the_brief_and_resume_from_the_status() {
  let paths = paths();
  let start = prompt_text(&paths, "main", false, false);
  assert_eq!(
    start,
    "You are an epic worker. Read /state/briefs/comemory-255.md and follow it exactly, starting at Pipeline step 1. Report status with the command it gives."
  );
  let resume = prompt_text(&paths, "dev", true, true);
  assert!(
    resume.contains("First inspect /state/status/comemory-255.json, `git log origin/dev..HEAD`")
  );
  assert!(resume.ends_with(" Your previous conversation for this issue is loaded above."));
}

#[test]
fn a_safe_worker_s_brief_says_its_approval_prompts_are_on() {
  let graph = graph_248();
  let issue = graph.find("Falconiere/comemory#255").expect("issue");
  let host = BriefHost {
    kind: HostKind::Codex,
    safe: true,
  };
  let safe = render_brief(&graph, issue, &paths(), "main", host).expect("brief");
  assert!(safe.contains("Approval prompts are on and a human may refuse a tool call"));
  assert!(!safe.contains("without approval prompts") && !safe.contains("{{"));
  let unattended = brief(&graph, "Falconiere/comemory#255", HostKind::Codex);
  assert!(unattended.contains("You run without approval prompts"));
  assert!(unattended.contains("Nobody answers questions in this pane."));
}

#[test]
fn the_worker_stops_after_reporting_babysit_and_waits_for_the_engine() {
  let graph = graph_248();
  for kind in crate::hosts::HOST_KINDS {
    let text = brief(&graph, "Falconiere/comemory#255", kind)
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" ");
    let babysit = skill_ref(kind, "pr-babysit", "babysit");
    assert!(
      text.contains("run `report babysit --pr <number>` instead and stop"),
      "{kind:?}"
    );
    assert!(text.contains(&format!("Do not start {babysit}, a cron or a polling loop")));
    assert!(text.contains("`pr-babysit: <repo>#<n> needs you (...)`"));
    assert!(text.contains("the engine marks the issue `ready`"));
    assert!(
      !text.contains("`report ready") && !text.contains("on a cron."),
      "{kind:?}"
    );
  }
}
