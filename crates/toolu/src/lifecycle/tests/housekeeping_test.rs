use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::housekeeping;

#[test]
fn a_statusline_symlink_is_removed_and_a_regular_file_stays() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("cfg");
  let toolu = root.join("toolu");
  std::fs::create_dir_all(&toolu).unwrap();
  let legacy = toolu.join("statusline.sh");
  std::fs::write(&legacy, "echo").unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", root.to_str().unwrap()),
    ("TOOLU_HOST_OVERRIDE", "claude"),
  ]);
  let roots = Roots::new(env.clone(), Some(Host::Claude));
  housekeeping(&roots);
  assert!(legacy.is_file());
  std::fs::remove_file(&legacy).unwrap();
  std::os::unix::fs::symlink("gone", &legacy).unwrap();
  housekeeping(&Roots::new(env, Some(Host::Claude)));
  assert!(std::fs::symlink_metadata(&legacy).is_err());
}
