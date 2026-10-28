use super::{document, plain};
use crate::host::Target;
use crate::verbs::doctor::{Counts, Finding, HostReport, Level, Scope};

#[test]
fn plain_lists_each_finding_and_the_counts() {
  let sections = [HostReport {
    host: Target::Claude,
    findings: vec![
      Finding::ok(Scope::Plugin, Some("jev".into()), "installed"),
      Finding::warn(
        Scope::Host,
        None,
        "no toolu plugin is installed",
        Some("toolu plugins install --host claude".into()),
      ),
    ],
  }];
  let text = plain(&sections, Counts::of(&sections));
  assert!(
    text.contains("ok   plugin   jev              installed"),
    "{text}"
  );
  assert!(
    text.contains("warn host     -                no toolu plugin is installed; toolu plugins install --host claude"),
    "{text}"
  );
  assert!(text.contains("1 ok, 1 warn, 0 fail"), "{text}");
}

#[test]
fn document_names_the_namespace_and_levels() {
  let sections = [HostReport {
    host: Target::Muse,
    findings: vec![Finding::fail(
      Scope::Secrets,
      Some("secrets.json".into()),
      "unreadable",
      Some("ensure secrets.json is readable".into()),
    )],
  }];
  let counts = Counts::of(&sections);
  let doc = document(&sections, counts);
  assert_eq!(doc["namespace"], "plugins");
  assert_eq!(doc["verb"], "doctor");
  assert_eq!(doc["ok"], false);
  assert_eq!(doc["counts"]["fail"], 1);
  assert_eq!(doc["hosts"][0]["host"], "muse");
  assert_eq!(doc["hosts"][0]["findings"][0]["status"], Level::Fail.name());
}
