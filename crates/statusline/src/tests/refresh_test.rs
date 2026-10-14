use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::refresh;

/// Config root `dir`, project `dir/project`, so no test reads the process directory's settings.
fn roots(dir: &Path, host: Host) -> Roots {
  Roots::new(
    Env::from_pairs([
      ("TOOLU_CONFIG_DIR", dir.to_string_lossy().into_owned()),
      (
        "TOOLU_PROJECT_DIR",
        dir.join("project").to_string_lossy().into_owned(),
      ),
    ]),
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
fn migrates_legacy_settings_then_retires_the_old_path_at_the_next_refresh() {
  let temp = tempfile::tempdir().expect("temp directory");
  leave_old_paths(temp.path());
  let settings = temp.path().join("settings.json");
  let original = r#"{"statusLine":{"type":"command","command":"bash ~/.claude/statusline/statusline.sh"},"theme":"dark"}"#;
  fs::write(&settings, original).expect("settings");
  let roots = roots(temp.path(), Host::Claude);
  let first = refresh(&roots).expect("first refresh");
  assert!(first.migrated && !first.retired);
  assert!(first.notice.is_some());
  assert_eq!(
    fs::read_to_string(temp.path().join("settings.json.bak")).expect("backup"),
    original
  );
  assert!(fs::symlink_metadata(temp.path().join("statusline/statusline.sh")).is_ok());
  let second = refresh(&roots).expect("second refresh");
  assert!(second.retired && !second.migrated);
  assert!(second.notice.is_none());
  assert!(fs::symlink_metadata(temp.path().join("statusline/statusline.sh")).is_err());
  assert!(!temp.path().join("toolu/statusline-render.sh").exists());
  let third = refresh(&roots).expect("third refresh");
  assert!(!third.migrated && !third.retired && third.notice.is_none());
}

#[test]
fn project_settings_naming_the_old_path_keep_it_and_name_the_file() {
  let temp = tempfile::tempdir().expect("temp directory");
  leave_old_paths(temp.path());
  let local = temp.path().join("project/.claude/settings.local.json");
  fs::create_dir_all(local.parent().expect("parent")).expect("project settings dir");
  fs::write(
    &local,
    r#"{"statusLine":{"type":"command","command":"bash ~/.claude/statusline/statusline.sh"}}"#,
  )
  .expect("local settings");
  let done = refresh(&roots(temp.path(), Host::Claude)).expect("refresh");
  assert!(!done.retired && !done.migrated);
  assert_eq!(
    done.notice.expect("notice"),
    format!(
      "statusline: {} still runs statusline/statusline.sh, which toolu no longer publishes; \
       set its statusLine command to toolu statusline render",
      local.display()
    )
  );
  assert!(fs::symlink_metadata(temp.path().join("statusline/statusline.sh")).is_ok());
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

#[test]
fn unreadable_project_settings_keep_the_old_path() {
  let temp = tempfile::tempdir().expect("temp directory");
  leave_old_paths(temp.path());
  let shared = temp.path().join("project/.claude/settings.json");
  fs::create_dir_all(shared.parent().expect("parent")).expect("project settings dir");
  fs::write(&shared, "{ not json").expect("broken settings");
  let done = refresh(&roots(temp.path(), Host::Claude)).expect("refresh");
  assert!(!done.retired && !done.migrated);
  let notice = done.notice.expect("notice");
  assert!(
    notice.starts_with(&format!("statusline: could not parse {}", shared.display()))
      && notice.ends_with("; statusline/statusline.sh stays until it can be read"),
    "{notice}"
  );
  assert!(fs::symlink_metadata(temp.path().join("statusline/statusline.sh")).is_ok());
}
