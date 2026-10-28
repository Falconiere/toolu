use std::path::{Path, PathBuf};

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{Launch, PLUGIN, command, run, start};

fn matches(args: &[&str]) -> clap::ArgMatches {
  command().try_get_matches_from(args).unwrap()
}

#[test]
fn the_sandbox_crate_is_its_plugin() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(command().get_name(), PLUGIN);
}

#[test]
fn json_does_not_apply_to_the_verbs_that_hand_over_the_terminal() {
  let ctx = Ctx {
    json: true,
    ..Ctx::default()
  };
  for args in [&["sandbox", "run", "true"][..], &["sandbox", "shell"]] {
    let outcome = run(&matches(args), &ctx);
    assert_eq!(outcome.exit, Exit::Usage);
    assert_eq!(outcome.stdout, None);
    let line = outcome.stderr.unwrap();
    assert!(
      line.starts_with("sandbox: --json does not apply to `"),
      "{line}"
    );
  }
}

#[test]
fn no_verb_is_a_usage_error() {
  let outcome = run(&clap::ArgMatches::default(), &Ctx::default());
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some("sandbox: a verb is required")
  );
}

#[test]
fn a_command_that_cannot_start_exits_69_naming_it() {
  let launch = Launch {
    argv: vec!["toolu-sandbox-no-such-program".to_owned()],
    cwd: PathBuf::from("/"),
  };
  let outcome = start(&launch);
  assert_eq!(outcome.exit, Exit::Unavailable);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some("sandbox: cannot start the command: toolu-sandbox-no-such-program: not found on PATH")
  );
}
