use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::refresh;

fn roots(dir: &Path, host: Host) -> Roots {
  Roots::new(
    Env::from_pairs([("TOOLU_CONFIG_DIR", dir.to_string_lossy())]),
    Some(host),
  )
}

/// The link and generated shim an older version left behind.
fn leave_old_paths(dir: &Path) {
  fs::create_dir_all(dir.join("statusline")).expect("statusline dir");
  fs::create_dir_all(dir.join("toolu")).expect("toolu dir");
  let generated = dir.join("toolu/statusline-render.sh");
  fs::write(
    &generated,
    "#!/bin/sh\nexec toolu statusline render \"$@\"\n",
  )
  .expect("generated shim");
  symlink(&generated, dir.join("statusline/statusline.sh")).expect("old link");
}

#[test]
fn migrates_legacy_settings_once_then_retires_the_old_path() {
  let temp = tempfile::tempdir().expect("temp directory");
  leave_old_paths(temp.path());
  let settings = temp.path().join("settings.json");
  let original = r#"{"statusLine":{"type":"command","command":"bash ~/.claude/statusline/statusline.sh"},"theme":"dark"}"#;
  fs::write(&settings, original).expect("settings");
  let roots = roots(temp.path(), Host::Claude);
  let first = refresh(&roots).expect("first refresh");
  assert!(first.retired);
  assert!(first.migrated);
  assert!(first.notice.is_some());
  assert_eq!(
    fs::read_to_string(temp.path().join("settings.json.bak")).expect("backup"),
    original
  );
  assert!(fs::symlink_metadata(temp.path().join("statusline/statusline.sh")).is_err());
  assert!(!temp.path().join("toolu/statusline-render.sh").exists());
  let second = refresh(&roots).expect("second refresh");
  assert!(!second.migrated && !second.retired);
  assert!(second.notice.is_none());
}

#[test]
fn a_custom_command_naming_the_old_path_keeps_it_and_says_so() {
  let temp = tempfile::tempdir().expect("temp directory");
  leave_old_paths(temp.path());
  let custom =
    r#"{"statusLine":{"type":"command","command":"~/.claude/statusline/statusline.sh | head -1"}}"#;
  fs::write(temp.path().join("settings.json"), custom).expect("settings");
  let done = refresh(&roots(temp.path(), Host::Claude)).expect("refresh");
  assert!(!done.retired && !done.migrated);
  let notice = done.notice.expect("notice");
  assert!(notice.contains("toolu statusline render"), "{notice}");
  assert!(fs::symlink_metadata(temp.path().join("statusline/statusline.sh")).is_ok());
}

#[test]
fn another_host_retires_a_plugin_link_and_keeps_a_user_file() {
  let temp = tempfile::tempdir().expect("temp directory");
  let plugin_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/statusline");
  fs::create_dir_all(temp.path().join("statusline")).expect("statusline dir");
  let link = temp.path().join("statusline/statusline.sh");
  symlink(plugin_root.join("scripts/statusline.sh"), &link).expect("old link");
  let roots = roots(temp.path(), Host::Cursor);
  let done = refresh(&roots).expect("refresh");
  assert!(done.retired && !done.migrated);
  assert!(fs::symlink_metadata(&link).is_err());
  fs::write(&link, "#!/bin/sh\necho mine\n").expect("user file");
  assert!(!refresh(&roots).expect("again").retired);
  assert_eq!(
    fs::read_to_string(&link).expect("kept"),
    "#!/bin/sh\necho mine\n"
  );
}
