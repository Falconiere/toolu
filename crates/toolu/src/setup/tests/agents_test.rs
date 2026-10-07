use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

#[test]
fn an_unknown_verb_prints_usage_and_writes_nothing() {
  let outcome = super::run(&Ctx::default(), &["deploy".to_owned()]);
  assert_eq!(outcome.exit, Exit::Blocked);
  assert_eq!(outcome.stdout, None);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some("Usage: toolu setup agents preview | install [--force] | remove --yes [--force]")
  );
}
