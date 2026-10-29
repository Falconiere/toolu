use super::{args, configure, load};
use clap::Command;
use toolu_runtime::env::Env;

#[test]
fn paired_targets_are_validated_and_cannot_drift() {
  let command = || Command::new("start").args(args());
  assert!(
    command()
      .try_get_matches_from(["start", "--machine", "vps"])
      .is_err()
  );
  let matches = command()
    .try_get_matches_from([
      "start",
      "--machine",
      "vps",
      "--remote-repo",
      "/repo with spaces",
    ])
    .unwrap();
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_EPIC_MULTIPLEXER", "herdr-app")]);
  configure(&matches, dir.path(), &env).unwrap();
  assert_eq!(load(dir.path()).unwrap()["machine"], "vps");
  configure(&matches, dir.path(), &env).unwrap();
  let different = command()
    .try_get_matches_from(["start", "--machine", "vps", "--remote-repo", "/other"])
    .unwrap();
  assert!(configure(&different, dir.path(), &env).is_err());
  assert!(
    configure(
      &matches,
      dir.path(),
      &env.with("TOOLU_EPIC_MULTIPLEXER", "tmux")
    )
    .is_err()
  );
}

#[test]
fn missing_target_is_local_but_malformed_target_is_not() {
  let dir = tempfile::tempdir().unwrap();
  assert!(load(dir.path()).unwrap().is_null());
  std::fs::write(dir.path().join("target.json"), "{}").unwrap();
  assert!(load(dir.path()).is_err());
}
