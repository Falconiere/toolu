//! The notify verb parses its closed kind set and addressing flags.

use super::NOTIFY_KINDS;

#[test]
fn notify_parses_kinds_and_optional_addressing() {
  let matches = super::command()
    .try_get_matches_from([
      "notify",
      "--kind",
      "contract-changed",
      "--status-file",
      "s/status/a.json",
      "--to",
      "b",
      "--note",
      "hi",
    ])
    .expect("parse");
  assert_eq!(
    matches.get_one::<String>("kind").expect("kind"),
    "contract-changed"
  );
  assert_eq!(matches.get_one::<String>("to").expect("to"), "b");
  assert!(NOTIFY_KINDS.contains(&"stuck"));
  assert!(
    super::command()
      .try_get_matches_from([
        "notify",
        "--status-file",
        "s/status/a.json",
        "--kind",
        "nope"
      ])
      .is_err()
  );
}
