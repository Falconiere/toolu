//! The epic-orchestrator Markdown runs `toolu epic` only, and the brief the
//! real binary renders fills every placeholder for each host and tracker.

#[path = "helpers/epic.rs"]
mod epic;
#[path = "helpers/epic_doc.rs"]
mod epic_doc;
#[path = "helpers/epic_graph.rs"]
mod epic_graph;

use std::collections::BTreeSet;
use std::path::Path;

use epic::{Home, Res, err};
use epic_doc::doc;
use epic_graph::write_graph;

/// The orchestrator's skill, its references and command, as the plugin ships them.
fn surfaces() -> Res<Vec<(String, String)>> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/epic-orchestrator");
  [
    "skills/epic-orchestrator/SKILL.md",
    "skills/epic-orchestrator/references/recovery.md",
    "skills/epic-orchestrator/references/worker-brief.md",
    "commands/epic.md",
  ]
    .iter()
    .map(|rel| Ok(((*rel).to_owned(), std::fs::read_to_string(root.join(rel))?)))
    .collect()
}

/// The verbs `toolu epic --help` lists.
fn verbs(home: &Home) -> Res<BTreeSet<String>> {
  let help = String::from_utf8(home.run(&["epic", "--help"])?.stdout)?;
  let commands = help
    .split("Commands:")
    .nth(1)
    .ok_or("no commands in the help")?;
  Ok(
    commands
      .lines()
      .skip(1)
      .take_while(|line| !line.trim().is_empty())
      .filter_map(|line| line.split_whitespace().next())
      .map(str::to_owned)
      .collect(),
  )
}

#[test]
fn the_skill_and_command_run_toolu_epic_verbs_only() {
  let home = Home::new().unwrap();
  let known = verbs(&home).unwrap();
  for (rel, text) in surfaces().unwrap() {
    for banned in ["bun ", "scripts/", "hooks/dist", ".ts`", "$S/"] {
      assert!(!text.contains(banned), "{rel} names {banned:?}");
    }
    let named: Vec<&str> = text
      .split("toolu ")
      .skip(1)
      .filter_map(|rest| rest.trim_start_matches("--json ").strip_prefix("epic "))
      .filter_map(|rest| rest.split(|ch: char| !ch.is_ascii_lowercase()).next())
      .filter(|verb| !verb.is_empty())
      .collect();
    assert!(!named.is_empty(), "{rel} names no toolu epic verb");
    for verb in named {
      assert!(
        known.contains(verb),
        "{rel}: `toolu epic {verb}` is not a verb"
      );
    }
  }
}

#[test]
fn the_preflight_checks_the_native_binary() {
  let skill = &surfaces().unwrap()[0].1;
  let preflight = skill
    .split("## 1. Preflight")
    .nth(1)
    .and_then(|rest| rest.split("\n## ").next())
    .unwrap();
  assert!(preflight.contains("Run `toolu doctor`"), "{preflight}");
  assert!(!preflight.contains("bun"), "{preflight}");
}

/// The brief `toolu epic launch --dry-run` renders for `kind` on `tracker`.
fn brief(home: &Home, kind: &str, tracker: Option<&str>) -> Res<String> {
  let graph = write_graph(home, |graph| {
    if let (Some(tracker), Some(map)) = (tracker, graph.as_object_mut()) {
      map.insert("tracker".to_owned(), serde_json::json!(tracker));
    }
  })?;
  let args = [
    "--json",
    "epic",
    "launch",
    "--graph",
    &graph,
    "--issue",
    "comemory-255",
    "--dry-run",
    "--kind",
    kind,
  ];
  let output = home.run(&args)?;
  if output.status.code() != Some(0) {
    return Err(err(&output).into());
  }
  let plan = doc(&output)?;
  let text = plan.get("brief").and_then(serde_json::Value::as_str);
  Ok(text.ok_or("no brief")?.to_owned())
}

#[test]
fn the_brief_fills_every_placeholder_for_each_host_and_tracker() {
  let home = Home::new().unwrap();
  for kind in ["claude", "codex", "cursor", "opencode"] {
    let text = brief(&home, kind, None).unwrap();
    assert!(!text.contains("{{"), "{kind}: {text}");
    assert!(
      text.contains("toolu epic report <phase> --status-file "),
      "{kind}"
    );
    assert!(text.contains("toolu epic job -- "), "{kind}");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
      flat.contains("`report babysit --pr <number>` instead and stop"),
      "{kind}"
    );
    assert!(!flat.contains("`report ready"), "{kind}");
  }
  let jira = brief(&home, "claude", Some("jira")).unwrap();
  assert!(jira.contains("`toolu epic jira issue get Falconiere/comemory#255`"));
  let linear = brief(&home, "claude", Some("linear")).unwrap();
  assert!(!linear.contains("{{") && !linear.contains("gh issue view"));
}
