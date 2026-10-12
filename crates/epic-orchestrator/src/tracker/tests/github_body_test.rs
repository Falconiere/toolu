use super::tick_body;

const BODY_248: &str = include_str!("../../../../../fixtures/epic/epic248-body.md");
const BODY_203: &str = include_str!("../../../../../fixtures/epic/epic203-body.md");
const TITLE: &str = "Define shared TS core contracts and verify OpenCode capabilities";

fn changed(before: &str, after: &str) -> Vec<(String, String)> {
  before
    .split('\n')
    .zip(after.split('\n'))
    .filter(|(a, b)| a != b)
    .map(|(a, b)| (a.to_owned(), b.to_owned()))
    .collect()
}

fn tick(body: &str, repo: (&str, &str), issue: &str, title: &str) -> String {
  tick_body(body, repo, issue, title).expect("tick")
}

#[test]
fn ticks_exactly_the_issue_line() {
  let next = tick(
    BODY_248,
    ("Falconiere", "comemory"),
    "Falconiere/comemory#255",
    "",
  );
  let lines = changed(BODY_248, &next);
  assert_eq!(lines.len(), 1);
  assert!(
    lines[0]
      .1
      .starts_with("- [x] https://github.com/Falconiere/comemory/issues/255 ")
  );
}

#[test]
fn ticks_a_cross_repo_issue_line() {
  let next = tick(
    BODY_248,
    ("Falconiere", "comemory"),
    "CodaSignal/comemory.io#183",
    "",
  );
  assert_eq!(changed(BODY_248, &next).len(), 1);
}

#[test]
fn a_prefix_number_does_not_match() {
  let next = tick(
    BODY_248,
    ("Falconiere", "comemory"),
    "Falconiere/comemory#25",
    "",
  );
  assert_eq!(next, BODY_248);
}

#[test]
fn table_mentions_are_untouched() {
  let next = tick(
    BODY_248,
    ("Falconiere", "comemory"),
    "Falconiere/comemory#257",
    "",
  );
  assert_eq!(changed(BODY_248, &next).len(), 1);
}

#[test]
fn a_bare_acceptance_criteria_line_is_not_ticked() {
  assert!(BODY_203.contains("- [ ] #205 records"));
  let next = tick(
    BODY_203,
    ("Falconiere", "toolu"),
    "Falconiere/toolu#205",
    TITLE,
  );
  assert_eq!(next, BODY_203);
}

#[test]
fn a_bare_tracking_line_with_its_title_or_alone_is_ticked() {
  let extended = format!("{BODY_203}\n- [ ] #205 {TITLE}\n- [ ] #205\n");
  let next = tick(
    &extended,
    ("Falconiere", "toolu"),
    "Falconiere/toolu#205",
    TITLE,
  );
  assert!(next.contains(&format!("- [x] #205 {TITLE}")));
  assert!(next.ends_with("- [x] #205\n"));
  assert!(next.contains("- [ ] #205 records"));
}

#[test]
fn the_bare_form_is_ignored_for_another_repository() {
  let line = format!("- [ ] #205 {TITLE}\n");
  let next = tick(
    &line,
    ("Falconiere", "comemory"),
    "Falconiere/toolu#205",
    TITLE,
  );
  assert_eq!(next, line);
}

#[test]
fn a_bad_issue_reference_is_refused() {
  assert!(tick_body("x", ("o", "r"), "nonsense", "").is_err());
}
