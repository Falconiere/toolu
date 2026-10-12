use crate::route::{Route, render_routes};

fn route(reference: &str, host: Option<&str>, score: Option<f64>) -> Route {
  Route {
    key: "k".into(),
    reference: reference.into(),
    tier: "standard".into(),
    score,
    confidence: None,
    source: "heuristic".into(),
    host: host.map(str::to_owned),
    model: None,
    effort: None,
    decided_at: "2026-10-11T00:00:00Z".into(),
  }
}

#[test]
fn rows_name_none_hosts_and_the_jev_note() {
  let text = render_routes(
    &[
      route("o/r#1", Some("codex"), Some(1.234)),
      route("o/r#2", None, None),
    ],
    Some("jev unavailable"),
  );
  let lines: Vec<&str> = text.lines().collect();
  assert!(lines[0].starts_with("issue"));
  assert!(lines[1].contains(" 1.23 codex "));
  assert!(lines[1].contains("(host default)"));
  assert!(lines[2].contains(" NONE "));
  assert_eq!(
    lines[3],
    "NONE = no host has free capacity; wait for a slot."
  );
  assert_eq!(lines[4], "jev unavailable");
  assert_eq!(render_routes(&[], None).lines().count(), 1);
}
