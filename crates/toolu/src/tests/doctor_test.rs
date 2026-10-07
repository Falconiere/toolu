use std::path::Path;

use super::{command, reachable};
use toolu_protocol::install::{BREW_UPGRADE, INSTALLER};

#[test]
fn doctor_is_a_command_without_a_planned_verb() {
  assert!(command().try_get_matches_from(["doctor"]).is_ok());
  assert!(
    command()
      .try_get_matches_from(["doctor", "planned"])
      .is_err()
  );
}

#[test]
fn doctor_names_the_upgrade_command_for_the_install_location() {
  // Neither path exists, so each is judged as given.
  let brew = reachable(Path::new("/nonexistent/Cellar/toolu/9.0.0/bin/toolu"));
  assert!(
    brew.ends_with(&format!("\nupgrade with: {BREW_UPGRADE}")),
    "{brew}"
  );
  let plain = reachable(Path::new("/nonexistent/plain/bin/toolu"));
  assert!(
    plain.ends_with(&format!("\nupgrade with: {INSTALLER}")),
    "{plain}"
  );
  assert!(!plain.contains(BREW_UPGRADE));
}
