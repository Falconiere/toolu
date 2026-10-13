//! The epic-orchestrator skill's shape for the engine (#452): small, it
//! starts and waits and never runs what the engine owns, it pairs every
//! judgment with a command, and the command and recovery files wrap the CLI.

use std::path::Path;

/// A fallible helper result.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// One shipped epic-orchestrator file.
fn shipped(rel: &str) -> Res<String> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/epic-orchestrator");
  Ok(std::fs::read_to_string(root.join(rel))?)
}

fn skill() -> Res<String> {
  shipped("skills/epic-orchestrator/SKILL.md")
}

/// The skill after its front matter.
fn body(text: &str) -> Res<&str> {
  Ok(text.splitn(3, "---\n").nth(2).ok_or("no front matter")?)
}

/// The text of the `## <title>` section, up to the next `## `.
fn section<'a>(text: &'a str, title: &str) -> Res<&'a str> {
  let (_, rest) = text
    .split_once(&format!("\n## {title}"))
    .ok_or_else(|| format!("no section {title}"))?;
  Ok(rest.split("\n## ").next().unwrap_or(rest))
}

#[test]
fn the_skill_body_is_under_150_lines() {
  let text = skill().unwrap();
  let lines = body(&text).unwrap().lines().count();
  assert!(lines < 150, "the skill body has {lines} lines");
}

#[test]
fn the_skill_runs_nothing_the_engine_owns() {
  let text = skill().unwrap();
  for owned in [
    "toolu epic gate",
    "toolu epic launch",
    "toolu epic finish",
    "toolu epic close",
    "gh pr merge",
    "--merge",
  ] {
    assert!(!text.contains(owned), "the skill names {owned:?}");
  }
  assert_eq!(text.matches("The engine merges, not you").count(), 1);
}

#[test]
fn the_skill_checks_previews_then_starts() {
  let text = skill().unwrap();
  let preflight = section(&text, "1. Preflight").unwrap();
  assert!(preflight.contains("`toolu doctor`"), "{preflight}");
  let plan = section(&text, "2. Plan").unwrap();
  assert!(
    plan.contains("epic graph <epic> --max 3 --dry-run"),
    "{plan}"
  );
  assert!(plan.contains("toolu epic route --graph /tmp/epic-graph.json"));
  assert!(plan.contains("--dry-run\n```") && plan.contains("A dry run stops here"));
  let run = section(&text, "3. Run").unwrap();
  let order = [
    "toolu --json epic graph <epic> --max 3 --save | jq -r .state_dir",
    "toolu epic route --graph \"$STATE_DIR/graph.json\"",
    "toolu epic start \"$STATE_DIR\"",
  ];
  let found: Vec<usize> = order.iter().map(|cmd| run.find(cmd).unwrap()).collect();
  assert!(found.windows(2).all(|pair| pair[0] < pair[1]), "{run}");
}

#[test]
fn the_skill_waits_in_the_background_on_claude_and_in_bounded_loops_elsewhere() {
  let text = skill().unwrap();
  let wait = section(&text, "4. Wait for judgments").unwrap();
  let claude = wait.split("- **Codex and OpenCode:**").next().unwrap();
  assert!(claude.contains("`toolu --json epic wait`") && claude.contains("run_in_background"));
  assert!(claude.contains("end your turn") && claude.contains("`waiting` after 45 minutes"));
  let others = wait.split("- **Codex and OpenCode:**").nth(1).unwrap();
  assert!(others.contains("`toolu --json epic wait --max-seconds 480`"));
  assert!(others.contains("run it again while any issue is active"));
}

#[test]
fn every_judgment_the_engine_raises_has_a_command() {
  let text = skill().unwrap();
  for kind in [
    "needs-human",
    "failed",
    "blocked",
    "stall",
    "babysit",
    "rebase",
    "coupling-hold",
    "relaunch-limit",
    "reopened",
    "pr-closed",
    "cleanup-incomplete",
    "refresh-failed",
    "herdr-error",
    "dependency-cycle",
    "merge-ready",
    "epic-complete",
  ] {
    let row = text
      .lines()
      .find(|line| line.starts_with(&format!("| `{kind}` |")))
      .unwrap_or_else(|| panic!("no row for {kind}"));
    let acts = ["toolu epic ", "references/recovery.md", "herdr status"];
    assert!(acts.iter().any(|act| row.contains(act)), "{row}");
  }
}

#[test]
fn status_reads_without_waiting_and_stop_pauses() {
  let text = skill().unwrap();
  let status = section(&text, "Status and stop").unwrap();
  assert!(status.contains("`toolu epic status <epic-key>`"));
  assert!(status.contains("`http://127.0.0.1:7717/`"));
  assert!(status.contains("never `toolu epic wait`"));
  assert!(status.contains("`toolu epic pause <epic-key>`"));
}

#[test]
fn the_command_wraps_one_cli_command_per_subcommand() {
  let text = shipped("commands/epic.md").unwrap();
  for (sub, command) in [
    ("start", "toolu epic start <state_dir>"),
    ("status", "toolu epic status"),
    ("pause", "toolu epic pause <epic-key>"),
    ("resume", "toolu epic resume <epic-key>"),
    ("stop", "toolu epic pause <epic-key>"),
    ("serve", "toolu epic serve"),
  ] {
    let row = text
      .lines()
      .find(|line| line.starts_with(&format!("| `{sub}")))
      .unwrap_or_else(|| panic!("no row for {sub}"));
    assert!(row.contains(command), "{row}");
  }
}

#[test]
fn recovery_starts_from_status_the_journal_and_doctor() {
  let text = shipped("skills/epic-orchestrator/references/recovery.md").unwrap();
  let first = text.split("\n## ").next().unwrap();
  for start in ["toolu epic status <key>", "/api/v1/events", "toolu doctor"] {
    assert!(first.contains(start), "{start}");
  }
  for repair in ["--reprompt", "--replace", "--abandon", "toolu epic gate"] {
    assert!(text.contains(repair), "{repair}");
  }
}
