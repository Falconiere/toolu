use super::*;

#[test]
fn bash_round_parsing_keeps_octal_boundary() {
  assert!(over_cap("010"));
  assert!(!over_cap("09"));
  assert!(!over_cap("5"));
}

#[test]
fn a_v1_state_gets_the_upgrade_message() {
  let doc = Ordered::parse("{\"version\":1}").expect("json");
  let failed = state_failure(&doc, "state.json", "abc", "main", "").expect("failure");
  assert_eq!(failed.code, "schema-v1");
}
