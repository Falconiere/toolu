use toolu_protocol::exit::Exit;

use super::{Selection, read};
use crate::host::Target;
use crate::support::{Sandbox, names};

fn replay(text: &str, version: &str) -> Result<Selection, (Exit, String)> {
  let sb = Sandbox::new();
  let path = sb.write(".toolu/plugins.json", text);
  read(&path, version).map_err(|fail| {
    (
      fail.exit,
      fail.message.replace(&path.display().to_string(), "<file>"),
    )
  })
}

#[test]
fn a_valid_selection_replays_its_names_and_host() {
  let text = r#"{"version":1,"generatorVersion":"6.5.0","host":"claude","enabled":["toolu","rust-quality"],"extra":1}"#;
  assert_eq!(
    replay(text, "7.11.0"),
    Ok(Selection {
      host: Some(Target::Claude),
      enabled: names(&["toolu", "rust-quality"])
    })
  );
  assert_eq!(
    replay(r#"{"version":1,"enabled":[]}"#, "7.11.0"),
    Ok(Selection {
      host: None,
      enabled: Vec::new()
    })
  );
  assert!(
    replay(
      r#"{"version":1,"generatorVersion":"7.0.0","enabled":[]}"#,
      "7.11.0"
    )
    .is_ok()
  );
}

#[test]
fn versions_are_refused_rather_than_guessed() {
  assert_eq!(
    replay(r#"{"version":2,"enabled":[]}"#, "7.11.0"),
    Err((
      Exit::Usage,
      "<file> declares version 2; this CLI understands version 1".to_owned()
    ))
  );
  assert_eq!(
    replay(r#"{"enabled":[]}"#, "7.11.0"),
    Err((
      Exit::Usage,
      "<file> has no numeric \"version\" field".to_owned()
    ))
  );
  assert_eq!(
    replay(
      r#"{"version":1,"generatorVersion":"99.0.0","enabled":[]}"#,
      "7.11.0"
    ),
    Err((
      Exit::Usage,
      "<file> was written by toolu 99.0.0; this CLI is 7.11.0. Upgrade the CLI.".to_owned()
    ))
  );
}

#[test]
fn malformed_and_missing_files_are_usage_errors() {
  let cases = [
    (
      r#"{"version":1,"enabled":"toolu"}"#,
      "is malformed: \"enabled\" must be an array",
    ),
    (r#"{"version":1,"enabled":[""]}"#, "non-empty strings"),
    (
      r#"{"version":1,"host":"cursor","enabled":[]}"#,
      "\"host\" must be claude, codex or opencode",
    ),
    (
      r#"{"version":1,"host":3,"enabled":[]}"#,
      "\"host\" must be a string",
    ),
    (
      r#"{"version":1,"generatorVersion":"","enabled":[]}"#,
      "\"generatorVersion\"",
    ),
    ("{", "is not valid JSON"),
  ];
  for (text, detail) in cases {
    let Err((exit, message)) = replay(text, "7.11.0") else {
      panic!("{text} replayed");
    };
    assert_eq!(exit, Exit::Usage);
    assert!(message.contains(detail), "{text}: {message}");
  }
  let sb = Sandbox::new();
  let missing = read(&sb.path("none.json"), "7.11.0").map_err(|fail| (fail.exit, fail.message));
  assert!(missing.is_err_and(
    |(exit, message)| exit == Exit::Usage && message.starts_with("cannot read selection file: ")
  ));
}
