use std::path::PathBuf;
use std::time::Duration;

use super::super::checks::Status;
use super::{PROBE_TIMEOUT, Probe, binary, reachability};
use toolu_protocol::install::{BREW_UPGRADE, INSTALLER};

#[test]
fn binary_names_the_upgrade_command_for_the_install_location() {
  let cellar = PathBuf::from("/nonexistent/Cellar/toolu/9.0.0/bin/toolu");
  let brew = binary(&Probe::at(cellar));
  assert!(brew.summary.contains(BREW_UPGRADE), "{}", brew.summary);
  assert_eq!(brew.details["install"], "homebrew");
  assert_eq!(brew.details["upgrade"], BREW_UPGRADE);
  let plain = binary(&Probe::at(PathBuf::from("/nonexistent/plain/bin/toolu")));
  assert!(plain.summary.contains(INSTALLER), "{}", plain.summary);
  assert!(!plain.summary.contains(BREW_UPGRADE));
  assert_eq!(plain.details["install"], "installer");
}

#[test]
fn reachability_not_found_fails_without_a_path() {
  let check = reachability(&Probe::missed());
  assert_eq!(check.status, Status::Fail);
  assert_eq!(check.details["reachable"], false);
  assert!(check.details["path"].is_null());
  assert_eq!(check.details["shadowed"], false);
  assert!(check.summary.contains("non-login shell"));
}

#[test]
fn a_shadowed_path_is_not_native() {
  let probe = Probe {
    listed: Some(PathBuf::from("/tmp/toolu")),
    shadowed: true,
    ..Probe::missed()
  };
  let check = reachability(&probe);
  assert_eq!(check.status, Status::Fail);
  assert_eq!(check.details["shadowed"], true);
  assert_eq!(check.details["reachable"], false);
  assert_eq!(check.details["path"], "/tmp/toolu");
}

#[test]
fn the_shell_probe_deadline_is_one_second() {
  assert_eq!(PROBE_TIMEOUT, Duration::from_secs(1));
}
