use super::{argv, command};

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
