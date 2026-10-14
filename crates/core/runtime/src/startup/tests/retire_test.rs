use std::os::unix::fs::symlink;
use std::path::Path;

use super::{Retired, retire};
use crate::env::Env;
use crate::host::roots::Roots;

fn roots(config: &Path) -> Roots {
  Roots::new(
    Env::from_pairs([("TOOLU_CONFIG_DIR", config.display().to_string())]),
    None,
  )
}

#[test]
fn a_link_into_a_plugin_scripts_dir_is_removed() {
  let dir = tempfile::tempdir().unwrap();
  let plugin = dir
    .path()
    .join("plugins/cache/toolu/jev/7.11.0/scripts/jev.sh");
  std::fs::create_dir_all(plugin.parent().unwrap()).unwrap();
  std::fs::write(&plugin, "#!/bin/sh\n").unwrap();
  let config = dir.path().join("config");
  std::fs::create_dir_all(config.join("jev")).unwrap();
  symlink(&plugin, config.join("jev/jev.sh")).unwrap();
  let roots = roots(&config);
  assert_eq!(
    retire(&roots, "jev", "jev", "jev.sh", None),
    Retired::Removed
  );
  assert!(std::fs::symlink_metadata(config.join("jev/jev.sh")).is_err());
  assert_eq!(
    retire(&roots, "jev", "jev", "jev.sh", None),
    Retired::Absent
  );
}

#[test]
fn a_dangling_link_from_a_removed_plugin_version_is_removed() {
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("config");
  std::fs::create_dir_all(config.join("jev")).unwrap();
  symlink(
    "/gone/plugins/cache/toolu/jev/6.0.0/scripts/jev.sh",
    config.join("jev/jev.sh"),
  )
  .unwrap();
  assert_eq!(
    retire(&roots(&config), "jev", "jev", "jev.sh", None),
    Retired::Removed
  );
}

#[test]
fn user_files_and_foreign_links_are_kept() {
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("config");
  std::fs::create_dir_all(config.join("jev")).unwrap();
  let file = config.join("jev/jev.sh");
  std::fs::write(&file, "#!/bin/sh\nmy own\n").unwrap();
  let roots = roots(&config);
  assert_eq!(retire(&roots, "jev", "jev", "jev.sh", None), Retired::Kept);
  assert_eq!(
    std::fs::read_to_string(&file).unwrap(),
    "#!/bin/sh\nmy own\n"
  );
  std::fs::remove_file(&file).unwrap();
  symlink("/home/me/bin/my-jev", &file).unwrap();
  assert_eq!(retire(&roots, "jev", "jev", "jev.sh", None), Retired::Kept);
  assert!(std::fs::symlink_metadata(&file).is_ok());
  std::fs::remove_file(&file).unwrap();
  symlink("/home/me/proj/scripts/jev.sh", &file).unwrap();
  assert_eq!(retire(&roots, "jev", "jev", "jev.sh", None), Retired::Kept);
  assert!(std::fs::symlink_metadata(&file).is_ok());
}

#[test]
fn an_extra_target_counts_as_ours() {
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("config");
  std::fs::create_dir_all(config.join("statusline")).unwrap();
  let generated = config.join("toolu/statusline-render.sh");
  symlink(&generated, config.join("statusline/statusline.sh")).unwrap();
  let roots = roots(&config);
  let name = "statusline.sh";
  assert_eq!(
    retire(&roots, "statusline", "statusline", name, None),
    Retired::Kept
  );
  assert_eq!(
    retire(&roots, "statusline", "statusline", name, Some(&generated)),
    Retired::Removed
  );
}

#[test]
fn a_link_that_cannot_be_removed_is_a_warning() {
  use std::os::unix::fs::PermissionsExt as _;
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("config");
  let jev = config.join("jev");
  std::fs::create_dir_all(&jev).unwrap();
  symlink("/x/plugins/jev/scripts/jev.sh", jev.join("jev.sh")).unwrap();
  std::fs::set_permissions(&jev, std::fs::Permissions::from_mode(0o500)).unwrap();
  let retired = retire(&roots(&config), "jev", "jev", "jev.sh", None);
  std::fs::set_permissions(&jev, std::fs::Permissions::from_mode(0o700)).unwrap();
  let Retired::Failed(line) = retired else {
    panic!("expected a failure, got {retired:?}");
  };
  assert!(
    line.starts_with("jev: cannot remove the retired "),
    "{line}"
  );
}
