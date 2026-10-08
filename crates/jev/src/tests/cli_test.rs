use clap::error::ErrorKind;

use super::command;

fn parse(args: &[&str]) -> Result<clap::ArgMatches, clap::Error> {
  let mut words = vec!["jev"];
  words.extend_from_slice(args);
  command().try_get_matches_from(words)
}

#[test]
fn shared_flags_default_and_every_verb_takes_them() {
  for verb in [
    ["noul", "Urgent?"],
    ["choice", "Which?"],
    ["score", "How?"],
    ["ask", "q.json"],
  ] {
    let matches = parse(&[verb[0], "-s", "x", verb[1]]).unwrap();
    let (_, sub) = matches.subcommand().unwrap();
    assert_eq!(sub.get_one::<String>("model").unwrap(), "jev-latest");
    assert!(!sub.get_flag("raw"));
  }
  let matches = parse(&["noul", "-s", "x", "-m", "m", "--raw", "Urgent?"]).unwrap();
  let (_, sub) = matches.subcommand().unwrap();
  assert_eq!(sub.get_one::<String>("model").unwrap(), "m");
  assert!(sub.get_flag("raw"));
}

#[test]
fn id_defaults_to_q_and_ask_has_none() {
  let matches = parse(&["score", "-s", "x", "How?"]).unwrap();
  let (_, sub) = matches.subcommand().unwrap();
  assert_eq!(sub.get_one::<String>("id").unwrap(), "q");
  assert!(parse(&["ask", "-", "-s", "x", "--id", "z"]).is_err());
}

#[test]
fn options_and_levels_repeat_and_a_dash_is_a_value() {
  let matches = parse(&["choice", "-s", "-", "Which?", "-o", "a=A", "--option", "b"]).unwrap();
  let (_, sub) = matches.subcommand().unwrap();
  let options: Vec<&String> = sub.get_many::<String>("option").unwrap().collect();
  assert_eq!(options, ["a=A", "b"]);
  assert_eq!(sub.get_one::<String>("state").unwrap(), "-");
  let matches = parse(&["ask", "-", "-s", "x"]).unwrap();
  let (_, sub) = matches.subcommand().unwrap();
  assert_eq!(sub.get_one::<String>("questions").unwrap(), "-");
}

#[test]
fn usage_errors_are_clap_errors() {
  let kind = |args: &[&str]| parse(args).unwrap_err().kind();
  assert_eq!(
    kind(&[]),
    ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
  );
  assert_eq!(
    kind(&["noul", "Urgent?"]),
    ErrorKind::MissingRequiredArgument
  );
  assert_eq!(
    kind(&["noul", "-s", "x", "Urgent?", "--weight", "3"]),
    ErrorKind::UnknownArgument
  );
  assert_eq!(kind(&["wat"]), ErrorKind::InvalidSubcommand);
}
