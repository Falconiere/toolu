//! The scenarios of #416 (AC-7), the size cap and the parse budget.

use std::time::Instant;

use toolu_shell::analysis::{CommandOrigin, Tristate};
use toolu_shell::git::runs_git_subcommand;
use toolu_shell::writes::write_targets;
use toolu_shell::{MAX_SHELL_INPUT, PARSE_BUDGET, analyze};

fn argv(words: &[&str]) -> Vec<Option<String>> {
  words.iter().map(|word| Some((*word).to_owned())).collect()
}

#[test]
fn bash_c_inside_a_pipeline_is_read_as_its_own_line() {
  let analysis = analyze("git status && bash -c \"rm -rf x\" | head");
  let found: Vec<(Vec<Option<String>>, CommandOrigin, usize)> = analysis
    .commands
    .into_iter()
    .map(|c| (c.argv, c.origin, c.depth))
    .collect();
  assert_eq!(
    found,
    [
      (argv(&["git", "status"]), CommandOrigin::Line, 0),
      (argv(&["bash", "-c", "rm -rf x"]), CommandOrigin::Line, 0),
      (argv(&["rm", "-rf", "x"]), CommandOrigin::Shell, 1),
      (argv(&["head"]), CommandOrigin::Line, 0),
    ]
  );
}

#[test]
fn a_push_followed_by_an_unterminated_quote_is_reported() {
  let analysis = analyze("git push origin main; echo 'unterminated");
  assert_eq!(runs_git_subcommand(&analysis, "push"), Tristate::Yes);
}

#[test]
fn a_redirect_glued_to_its_target_writes_it() {
  let analysis = analyze("echo x >.env");
  let paths: Vec<Option<String>> = write_targets(&analysis)
    .into_iter()
    .map(|t| t.path)
    .collect();
  assert_eq!(paths, [Some(".env".to_owned())]);
}

#[test]
fn input_one_unit_over_the_cap_is_unknown_with_the_typescript_message() {
  let analysis = analyze(&"a".repeat(MAX_SHELL_INPUT + 1));
  assert!(analysis.unknown);
  assert_eq!(
    analysis.errors[0].message,
    "oversize: 1048577 characters exceeds the 1048576 cap"
  );
}

#[test]
fn a_pathological_megabyte_is_cancelled_at_the_budget() {
  let started = Instant::now();
  let analysis = analyze(&"${".repeat(524_288));
  let elapsed = started.elapsed();
  assert!(analysis.unknown);
  assert!(elapsed < PARSE_BUDGET * 3, "took {elapsed:?}");
}
