use std::path::Path;

use super::upgrade_command;
use toolu_protocol::install::{BREW_UPGRADE, INSTALLER};

/// These paths do not exist on a Linux test host, so they also exercise the
/// unresolvable-path fallback.
#[test]
fn homebrew_prefixes_upgrade_with_brew() {
  assert_eq!(
    upgrade_command(Path::new("/opt/homebrew/bin/toolu")),
    BREW_UPGRADE
  );
  assert_eq!(
    upgrade_command(Path::new("/home/linuxbrew/.linuxbrew/bin/toolu")),
    BREW_UPGRADE
  );
}

#[test]
fn the_installer_directories_upgrade_with_the_installer() {
  assert_eq!(
    upgrade_command(Path::new("/usr/local/bin/toolu")),
    INSTALLER
  );
  assert_eq!(
    upgrade_command(Path::new("/home/u/.local/bin/toolu")),
    INSTALLER
  );
}

#[test]
fn a_link_into_a_cellar_upgrades_with_brew() {
  let dir = tempfile::tempdir().unwrap();
  let cellar = dir.path().join("Cellar/toolu/7.10.0/bin");
  std::fs::create_dir_all(&cellar).unwrap();
  std::fs::write(cellar.join("toolu"), "").unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  std::os::unix::fs::symlink(cellar.join("toolu"), bin.join("toolu")).unwrap();
  assert_eq!(upgrade_command(&bin.join("toolu")), BREW_UPGRADE);
}
