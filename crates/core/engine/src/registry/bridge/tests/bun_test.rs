use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use toolu_runtime::env::Env;

use super::{BunState, claim_advisory, find_bun, session_file};

fn executable(path: &Path) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, "#!/bin/sh\n").unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn bun_is_looked_for_in_toolu_bun_then_path_then_home() {
  let dir = tempfile::tempdir().unwrap();
  let d = dir.path();
  let text = |rel: &str| d.join(rel).to_string_lossy().into_owned();
  executable(&d.join("explicit/bun"));
  executable(&d.join("path/bun"));
  executable(&d.join("home/.bun/bin/bun"));
  std::fs::write(d.join("plain/bun"), "").unwrap_or(());
  let env = |pairs: &[(&str, String)]| Env::from_pairs(pairs.iter().cloned());
  let all = env(&[
    ("TOOLU_BUN", text("explicit/bun")),
    ("PATH", text("path")),
    ("HOME", text("home")),
  ]);
  assert_eq!(find_bun(&all), Some(d.join("explicit/bun")));
  let no_explicit = env(&[
    ("TOOLU_BUN", text("missing")),
    ("PATH", format!("::{}", text("path"))),
    ("HOME", text("home")),
  ]);
  assert_eq!(find_bun(&no_explicit), Some(d.join("path/bun")));
  assert_eq!(
    find_bun(&env(&[("PATH", text("none")), ("HOME", text("home"))])),
    Some(d.join("home/.bun/bin/bun"))
  );
  assert_eq!(find_bun(&env(&[("HOME", text("none"))])), None);
  let mut state = BunState::default();
  assert_eq!(state.bun(&all), Some(d.join("explicit/bun")));
  assert_eq!(
    state.bun(&Env::default()),
    Some(d.join("explicit/bun")),
    "looked for once"
  );
}

#[test]
fn session_ids_become_safe_file_names() {
  assert_eq!(session_file("01a0-f1c1_x.y"), "01a0-f1c1_x.y");
  assert_eq!(session_file("../etc/passwd"), ".._etc_passwd");
  assert_eq!(session_file(""), "unknown");
  assert_eq!(session_file(".."), "unknown");
  assert_eq!(session_file(&"a".repeat(300)).len(), 128);
}

#[test]
fn the_advisory_is_claimed_once_per_session_marker() {
  let dir = tempfile::tempdir().unwrap();
  let markers = dir.path().join("registry-bridge");
  assert!(claim_advisory(Some(&markers), "s1"));
  assert!(!claim_advisory(Some(&markers), "s1"));
  assert!(claim_advisory(Some(&markers), "s2"));
  assert!(
    claim_advisory(None, "s1"),
    "no project state dir shows it every time"
  );
  let blocked = dir.path().join("file");
  std::fs::write(&blocked, "x").unwrap();
  assert!(claim_advisory(Some(&blocked), "s1"));
  assert!(
    claim_advisory(Some(&blocked), "s1"),
    "an unwritable marker repeats the advisory"
  );
}
