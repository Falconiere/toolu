use std::path::Path;

use toolu_jev_client::Config;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{PLUGIN, command, execute, needs_stdin, run};

#[test]
fn the_jev_crate_is_its_plugin_and_has_the_four_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let verbs: Vec<String> = command()
    .get_subcommands()
    .map(|verb| verb.get_name().to_owned())
    .collect();
  assert_eq!(verbs, ["noul", "choice", "score", "ask"]);
  assert!(command().try_get_matches_from(["jev", "planned"]).is_err());
}

#[test]
fn a_missing_key_fails_before_the_arguments_are_read() {
  let matches = command()
    .try_get_matches_from(["jev", "noul", "-s", "@/nonexistent/state", "Urgent?"])
    .unwrap();
  let outcome = execute(
    &matches,
    &Ctx::default(),
    &Env::default(),
    Config::default(),
    None,
  );
  assert_eq!(outcome.exit, Exit::Failure);
  assert_eq!(outcome.stdout, None);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some("jev: TYPESAFE_API_KEY unset")
  );
}

#[test]
fn needs_stdin_is_true_only_for_a_dash() {
  let wants = |args: &[&str]| {
    let mut argv = vec!["jev"];
    argv.extend_from_slice(args);
    needs_stdin(&command().try_get_matches_from(argv).unwrap())
  };
  assert!(wants(&["noul", "-s", "-", "Urgent?"]));
  assert!(wants(&["ask", "-", "-s", "ticket"]));
  assert!(!wants(&["noul", "-s", "ticket", "Urgent?"]));
  assert!(!wants(&["ask", "questions.json", "-s", "@state.json"]));
}

#[test]
fn run_reads_the_process_environment_and_never_stdin() {
  // Without a key in the test process's environment this fails before any
  // request; with one, `-s -` has no stdin to read and fails the same way.
  let matches = command()
    .try_get_matches_from(["jev", "noul", "-s", "-", "Urgent?"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Failure);
  assert_eq!(outcome.stdout, None);
}
