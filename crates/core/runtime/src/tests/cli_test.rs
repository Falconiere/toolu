use toolu_protocol::exit::Exit;

use super::{Ctx, Outcome};

#[test]
fn data_is_a_success_with_stdout_only() {
  let outcome = Outcome::data("toolu 7.11.0".to_owned());
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stdout.as_deref(), Some("toolu 7.11.0"));
  assert_eq!(outcome.stderr, None);
}

#[test]
fn failed_carries_its_exit_and_a_diagnostic_but_no_data() {
  let outcome = Outcome::failed(Exit::Unavailable, "gh is not installed".to_owned());
  assert_eq!(outcome.exit, Exit::Unavailable);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr.as_deref(), Some("gh is not installed"));
}

#[test]
fn the_default_context_has_no_flag_set() {
  let ctx = Ctx::default();
  assert!(!ctx.json && !ctx.quiet);
  assert_eq!(
    (
      ctx.host.as_ref(),
      ctx.config_dir.as_ref(),
      ctx.stdin.as_ref()
    ),
    (None, None, None)
  );
}

#[test]
fn a_notice_is_one_system_message_document_or_nothing() {
  let shown = Outcome::notice(Some("line \"one\"\nline two".to_owned()));
  assert_eq!(shown.exit, Exit::Success);
  assert_eq!(
    shown.stdout.as_deref(),
    Some(r#"{"systemMessage":"line \"one\"\nline two"}"#)
  );
  assert_eq!(shown.stderr, None);
  assert_eq!(Outcome::notice(None).stdout, None);
}
