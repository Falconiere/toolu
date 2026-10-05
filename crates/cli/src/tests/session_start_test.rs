use std::path::Path;

use super::diagnostic;

#[test]
fn startup_and_resume_name_the_version_and_path() {
  for source in ["startup", "resume"] {
    let payload = format!(r#"{{"hook_event_name":"SessionStart","source":"{source}"}}"#);
    assert_eq!(
      diagnostic(&payload, "7.10.0", Some(Path::new("/usr/local/bin/toolu"))).as_deref(),
      Some("toolu runtime: native 7.10.0 at /usr/local/bin/toolu")
    );
  }
  assert_eq!(
    diagnostic(r#"{"source":"startup"}"#, "1.0.0", None).as_deref(),
    Some("toolu runtime: native 1.0.0 at an unknown path")
  );
}

#[test]
fn other_sources_and_unreadable_payloads_say_nothing() {
  for payload in [
    r#"{"source":"clear"}"#,
    r#"{"source":1}"#,
    "{}",
    "",
    "{trunc",
    "[]",
  ] {
    assert_eq!(diagnostic(payload, "7.10.0", None), None, "{payload}");
  }
}
