use super::{Counts, Finding, Level, Scope};

#[test]
fn counts_sum_every_level() {
  let findings = [
    Finding::ok(Scope::Plugin, Some("jev".into()), "ok"),
    Finding::warn(
      Scope::Secrets,
      Some("TYPESAFE_API_KEY".into()),
      "unset",
      None,
    ),
    Finding::fail(Scope::Secrets, Some("secrets.json".into()), "bad", None),
  ];
  let mut counts = Counts::default();
  for finding in &findings {
    counts.add(finding.status);
  }
  assert_eq!(counts.line(), "1 ok, 1 warn, 1 fail");
  assert_eq!(Level::Ok.name(), "ok");
}
