use super::{Match, matches};

// Trimmed from real `ast-grep scan --json=compact` output (ast-grep 0.51.2).
const REAL: &str = r#"[{"text":"pub struct S { a: i32 }","range":{"byteOffset":{"start":36,"end":59},"start":{"line":2,"column":0},"end":{"line":2,"column":23}},"file":"a.rs","lines":"pub struct S { a: i32 }","language":"Rust","ruleId":"outline-rust","metaVariables":{"single":{"NAME":{"text":"S","range":{"byteOffset":{"start":47,"end":48},"start":{"line":2,"column":11},"end":{"line":2,"column":12}}}},"multi":{},"transformed":{}}}]"#;

#[test]
fn outline_parse_reads_file_range_and_name() {
  assert_eq!(
    matches(REAL).unwrap(),
    [Match {
      file: "a.rs".to_owned(),
      start: 2,
      end: 2,
      name: "S".to_owned(),
      bytes: (36, 59),
    }]
  );
}

#[test]
fn outline_parse_treats_blank_output_as_no_matches() {
  assert_eq!(matches("  \n").unwrap(), []);
}

#[test]
fn outline_parse_rejects_bad_json_and_nameless_matches() {
  assert_eq!(matches("{").unwrap_err(), "ast-grep printed invalid JSON");
  let nameless = REAL.replace("\"NAME\"", "\"OTHER\"");
  assert!(
    matches(&nameless)
      .unwrap_err()
      .contains("without a file, range or name")
  );
}
