use std::os::unix::fs::PermissionsExt as _;

use super::stage;

fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
  stage(path, text)?.commit()
}
use crate::support::Sandbox;

#[test]
fn a_write_through_a_symlink_edits_the_target_and_keeps_the_link() {
  let sb = Sandbox::new();
  let real = sb.write("real.json", "{}\n");
  let link = sb.path("link.json");
  std::os::unix::fs::symlink(&real, &link).expect("symlink");
  write_atomic(&link, "{ \"plugin\": [] }\n").expect("write");
  assert!(
    std::fs::symlink_metadata(&link)
      .expect("lstat")
      .file_type()
      .is_symlink()
  );
  assert_eq!(sb.read("real.json"), "{ \"plugin\": [] }\n");
}

#[test]
fn a_rewrite_keeps_permissions_and_a_new_file_gets_its_directories() {
  let sb = Sandbox::new();
  let secret = sb.write("secret.json", "{ \"provider\": {} }\n");
  std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).expect("chmod");
  write_atomic(&secret, "{ \"provider\": {}, \"plugin\": [] }\n").expect("write");
  assert_eq!(
    std::fs::metadata(&secret)
      .expect("stat")
      .permissions()
      .mode()
      & 0o777,
    0o600
  );
  write_atomic(&sb.path("deep/er/new.json"), "x").expect("new");
  assert_eq!(sb.read("deep/er/new.json"), "x");
  let leftovers: Vec<_> = std::fs::read_dir(sb.root())
    .expect("dir")
    .flatten()
    .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
    .collect();
  assert!(leftovers.is_empty());
  let blocked = sb.write("file", "");
  assert!(write_atomic(&blocked.join("child.json"), "x").is_err());
}

#[test]
fn a_discarded_or_failed_stage_leaves_no_temporary_file() {
  let sb = Sandbox::new();
  let staged = stage(&sb.path("cfg/a.json"), "x").expect("stage");
  staged.discard();
  assert!(!sb.path("cfg/a.json").exists());
  std::fs::create_dir_all(sb.path("cfg/dir/inner")).expect("dir");
  let onto_dir = stage(&sb.path("cfg/dir"), "x").expect("stage");
  assert!(onto_dir.commit().is_err());
  let temps: Vec<_> = std::fs::read_dir(sb.path("cfg"))
    .expect("cfg")
    .flatten()
    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
    .collect();
  assert_eq!(temps.len(), 0);
}

#[test]
fn a_symlink_planted_at_a_temporary_name_is_never_followed() {
  let sb = Sandbox::new();
  let victim = sb.write("victim.txt", "keep");
  let dir = sb.path("cfg");
  std::fs::create_dir_all(&dir).expect("cfg");
  let next = super::NEXT.load(std::sync::atomic::Ordering::Relaxed);
  for unique in next..next + 8 {
    let name = format!(".a.json.{}.{unique}.tmp", std::process::id());
    std::os::unix::fs::symlink(&victim, dir.join(name)).expect("plant");
  }
  write_atomic(&dir.join("a.json"), "new").expect("write");
  assert_eq!(sb.read("victim.txt"), "keep");
  assert_eq!(sb.read("cfg/a.json"), "new");
}
