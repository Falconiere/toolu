//! `toolu epic report` writing the status files the engine reads, from the
//! real binary (the six `report.ts` cases).

#[path = "helpers/epic.rs"]
mod epic;

use serde_json::Value;

use epic::{Home, Res, err};

fn status(home: &Home) -> Res<Value> {
  let text = std::fs::read_to_string(home.path("epic/status/k-1.json"))?;
  Ok(serde_json::from_str(&text)?)
}

fn report(home: &Home, args: &[&str]) -> Res<std::process::Output> {
  let file = home.path("epic/status/k-1.json").display().to_string();
  let mut line = vec!["epic", "report"];
  line.extend_from_slice(args);
  line.extend(["--status-file", &file]);
  home.run(&line)
}

#[test]
fn history_and_pr_persist_across_phases() {
  let home = Home::new().unwrap();
  for args in [
    vec!["brainstorm"],
    vec!["spec"],
    vec!["pr-open", "--pr", "301"],
    vec!["babysit"],
  ] {
    let output = report(&home, &args).unwrap();
    assert_eq!(output.status.code(), Some(0), "{args:?}: {}", err(&output));
  }
  let data = status(&home).unwrap();
  assert_eq!(
    (data["phase"].as_str(), data["pr"].as_u64()),
    (Some("babysit"), Some(301))
  );
  let phases: Vec<&str> = data["history"]
    .as_array()
    .expect("history")
    .iter()
    .filter_map(|row| row["phase"].as_str())
    .collect();
  assert_eq!(phases, ["brainstorm", "spec", "pr-open", "babysit"]);
  assert!(
    data["updated_at"]
      .as_str()
      .is_some_and(|at| at.len() == 20 && at.ends_with('Z'))
  );
}

#[test]
fn a_note_lands_on_the_status_and_its_history_then_clears() {
  let home = Home::new().unwrap();
  report(&home, &["needs-human", "--note", "which base branch?"]).unwrap();
  assert_eq!(status(&home).unwrap()["note"], "which base branch?");
  assert_eq!(
    status(&home).unwrap()["history"][0]["note"],
    "which base branch?"
  );
  report(&home, &["spec"]).unwrap();
  assert_eq!(status(&home).unwrap()["note"], "");
}

#[test]
fn the_engine_reads_what_report_writes() {
  let home = Home::new().unwrap();
  let output = report(&home, &["ready", "--pr", "301"]).unwrap();
  assert_eq!(output.status.code(), Some(0), "{}", err(&output));
  let spooled = std::fs::read_dir(home.path("resources/spool"))
    .expect("spool")
    .count();
  assert_eq!(spooled, 1, "the report waits in the spool for the engine");
  assert_eq!(status(&home).unwrap()["phase"], "ready");
}

#[test]
fn keys_a_previous_status_carried_are_kept() {
  let home = Home::new().unwrap();
  let file = home.path("epic/status/k-1.json");
  std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
  std::fs::write(
    &file,
    r#"{"extra":"kept","phase":"spec","pr":7,"history":[]}"#,
  )
  .expect("status");
  report(&home, &["plan"]).unwrap();
  let data = status(&home).unwrap();
  assert_eq!(
    (data["extra"].as_str(), data["pr"].as_u64()),
    (Some("kept"), Some(7))
  );
}

#[test]
fn a_corrupt_or_non_object_previous_status_starts_over() {
  let home = Home::new().unwrap();
  let file = home.path("epic/status/k-1.json");
  std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
  for body in ["{not json", "[1,2]", ""] {
    std::fs::write(&file, body).expect("status");
    let output = report(&home, &["spec"]).unwrap();
    assert_eq!(output.status.code(), Some(0), "{body}: {}", err(&output));
    let phases: Vec<Value> = status(&home).unwrap()["history"]
      .as_array()
      .cloned()
      .unwrap_or_default();
    assert_eq!(
      phases.last().and_then(|row| row["phase"].as_str()),
      Some("spec")
    );
  }
}

#[test]
fn usage_errors_exit_64_and_write_nothing() {
  let home = Home::new().unwrap();
  for args in [
    vec!["merged"],
    vec!["ready", "--pr", "abc"],
    vec!["ready", "--pr"],
    vec!["ready", "--note"],
    vec!["ready", "--bogus"],
  ] {
    let output = report(&home, &args).unwrap();
    assert_eq!(output.status.code(), Some(64), "{args:?}");
  }
  assert_eq!(
    home.run(&["epic", "report"]).unwrap().status.code(),
    Some(64)
  );
  assert!(!home.path("epic/status/k-1.json").exists());
}
