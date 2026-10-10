use super::{argv, command, savings};

#[test]
fn savings_cli_reports_oversized_ledger() {
  let file = tempfile::NamedTempFile::new().expect("temporary ledger");
  file
    .as_file()
    .set_len(crate::report::MAX_LEDGER_BYTES + 1)
    .expect("extend ledger");
  let path = file.path().to_str().expect("UTF-8 path");
  let parsed = command()
    .try_get_matches_from(["ast-grep", "savings", path])
    .expect("valid savings command");
  let (_, matches) = parsed.subcommand().expect("savings");
  let outcome = savings(matches, &toolu_runtime::cli::Ctx::default());
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Failure);
  assert!(
    outcome
      .stderr
      .expect("error")
      .contains("ledger exceeds 16777216 bytes")
  );
}

#[test]
fn cli_preserves_external_flags_after_pattern() {
  let parsed = command()
    .try_get_matches_from([
      "ast-grep",
      "search",
      "console.log($A)",
      "-l",
      "typescript",
      "src/app.ts",
    ])
    .expect("valid search");
  let (_, matches) = parsed.subcommand().expect("search");
  assert_eq!(
    argv("search", matches).expect("argv"),
    [
      "run",
      "--pattern",
      "console.log($A)",
      "--color",
      "never",
      "-l",
      "typescript",
      "src/app.ts"
    ]
  );
}
