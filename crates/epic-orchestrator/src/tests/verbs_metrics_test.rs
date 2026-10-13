use super::commands;

#[test]
fn the_verbs_parse_their_flags() {
  let tree = clap::Command::new("epic").subcommands(commands());
  let matches = tree
    .clone()
    .try_get_matches_from(["epic", "metrics", "--epic", "e1", "--since", "2026-10-01"])
    .unwrap();
  let (_, metrics) = matches.subcommand().unwrap();
  assert_eq!(metrics.get_one::<String>("since").unwrap(), "2026-10-01");
  let matches = tree
    .clone()
    .try_get_matches_from([
      "epic",
      "baseline",
      "--state",
      "s",
      "--until",
      "t",
      "--outlier",
      "a",
      "--outlier",
      "b",
    ])
    .unwrap();
  let (_, baseline) = matches.subcommand().unwrap();
  assert_eq!(baseline.get_many::<String>("outlier").unwrap().count(), 2);
  assert!(
    tree
      .try_get_matches_from(["epic", "baseline", "--state", "s"])
      .is_err()
  );
}
