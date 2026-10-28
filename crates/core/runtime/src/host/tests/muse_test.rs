use std::os::unix::fs::PermissionsExt as _;

use super::{config_dir, dir_unreadable, file_unreadable};
use crate::env::Env;

#[test]
fn config_dir_prefers_muse_then_xdg_then_home() {
  let muse = Env::from_pairs([("MUSE_CONFIG_DIR", "/muse"), ("HOME", "/home/u")]);
  assert_eq!(config_dir(&muse).as_os_str(), "/muse");

  let xdg = Env::from_pairs([("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/u")]);
  assert_eq!(config_dir(&xdg).as_os_str(), "/xdg/muse");

  let home = Env::from_pairs([("HOME", "/home/u")]);
  assert_eq!(config_dir(&home).as_os_str(), "/home/u/.config/muse");
}

#[test]
fn config_dir_ignores_toolu_config_dir() {
  let env = Env::from_pairs([
    ("HOME", "/home/u"),
    ("TOOLU_CONFIG_DIR", "/toolu"),
    ("MUSE_CONFIG_DIR", "/muse"),
  ]);
  assert_eq!(config_dir(&env).as_os_str(), "/muse");
}

#[test]
fn unreadable_probes_follow_permission_bits() {
  let root = tempfile::tempdir().expect("tempdir");
  let dir = root.path().join("cfg");
  std::fs::create_dir(&dir).expect("mkdir");
  let file = dir.join("settings.json");
  std::fs::write(&file, "{}").expect("write");

  assert!(!dir_unreadable(&dir));
  assert!(!file_unreadable(&file));

  let mut perms = std::fs::metadata(&file).expect("meta").permissions();
  perms.set_mode(0o000);
  std::fs::set_permissions(&file, perms).expect("chmod file");
  assert!(file_unreadable(&file));

  let mut perms = std::fs::metadata(&dir).expect("meta").permissions();
  perms.set_mode(0o000);
  std::fs::set_permissions(&dir, perms).expect("chmod dir");
  assert!(dir_unreadable(&dir));

  let mut perms = std::fs::metadata(&dir).expect("meta").permissions();
  perms.set_mode(0o700);
  std::fs::set_permissions(&dir, perms).expect("restore dir");
  let mut perms = std::fs::metadata(&file).expect("meta").permissions();
  perms.set_mode(0o600);
  std::fs::set_permissions(&file, perms).expect("restore file");
}
