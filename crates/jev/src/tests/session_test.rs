use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::session_start;
use crate::hooks::{KEY, Sandbox, context_of, install, mandate, skill_file};

const EVENT: &str = "SessionStart";

/// A link an older version published, into a plugin version's `scripts/`.
fn old_link(link: &Path, dir: &Path) {
  let script = dir.join("jev/7.11.0/scripts/jev.sh");
  fs::create_dir_all(script.parent().unwrap()).unwrap();
  fs::write(&script, "#!/bin/sh\nexec toolu jev \"$@\"\n").unwrap();
  fs::create_dir_all(link.parent().unwrap()).unwrap();
  symlink(&script, link).unwrap();
}

#[test]
fn states_the_mandate_naming_toolu_jev_on_claude() {
  let sb = Sandbox::new();
  let context = context_of(&session_start(&sb.env("claude"), &sb.plugin), EVENT);
  assert_eq!(context, mandate(&skill_file(&sb.plugin)));
  assert!(!context.contains(KEY) && !context.to_lowercase().contains("bun"));
  assert!(!context.contains("jev.sh"));
  assert!(!sb.claude_dir().exists(), "nothing is published");
}

#[test]
fn retires_the_old_link_under_codex_home_with_spaces() {
  let sb = Sandbox::new();
  let codex = sb.dir.path().join("codex profile");
  let link = codex.join("jev/jev.sh");
  old_link(&link, &sb.dir.path().join("cache"));
  let env = sb.env("codex").with("CODEX_HOME", codex.to_str().unwrap());
  let context = context_of(&session_start(&env, &sb.plugin), EVENT);
  assert_eq!(context, mandate(&skill_file(&sb.plugin)));
  assert!(fs::symlink_metadata(&link).is_err(), "the old link is gone");
  assert!(!sb.home.join(".claude").exists());
}

#[test]
fn toolu_config_dir_takes_precedence_over_the_codex_root() {
  let sb = Sandbox::new();
  let custom = sb.dir.path().join("custom profile");
  let codex = sb.dir.path().join("codex");
  old_link(&custom.join("jev/jev.sh"), &sb.dir.path().join("cache"));
  old_link(&codex.join("jev/jev.sh"), &sb.dir.path().join("cache2"));
  let env = sb.env("codex").with("CODEX_HOME", codex.to_str().unwrap());
  let env = env.with("TOOLU_CONFIG_DIR", custom.to_str().unwrap());
  context_of(&session_start(&env, &sb.plugin), EVENT);
  assert!(fs::symlink_metadata(custom.join("jev/jev.sh")).is_err());
  assert!(
    codex.join("jev/jev.sh").is_symlink(),
    "another root is untouched"
  );
}

#[test]
fn a_users_own_file_survives_byte_for_byte() {
  let sb = Sandbox::new();
  let file = sb.claude_dir().join("jev.sh");
  fs::create_dir_all(sb.claude_dir()).unwrap();
  fs::write(&file, "#!/bin/sh\necho mine\n").unwrap();
  fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
  let context = context_of(&session_start(&sb.env("claude"), &sb.plugin), EVENT);
  assert_eq!(fs::read_to_string(&file).unwrap(), "#!/bin/sh\necho mine\n");
  assert!(
    context.contains("you MUST call toolu jev before"),
    "{context}"
  );
}

#[test]
fn without_the_hook_key_the_command_environment_is_checked_first() {
  let sb = Sandbox::new();
  let env = Env::from_pairs([("HOME", sb.home.to_str().unwrap())]);
  let context = context_of(&session_start(&env, &sb.plugin), EVENT);
  assert!(context.starts_with("The Jev hook did not receive TYPESAFE_API_KEY."));
  assert!(context.contains(
    "check whether TYPESAFE_API_KEY is set in the command environment without printing its value"
  ));
  assert!(context.contains(&mandate(&skill_file(&sb.plugin))));
}

#[test]
fn a_link_that_cannot_be_removed_is_one_warning_beside_the_mandate() {
  let sb = Sandbox::new();
  let dir = sb.claude_dir();
  old_link(&dir.join("jev.sh"), &sb.dir.path().join("cache"));
  fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
  let outcome = session_start(&sb.env("claude"), &sb.plugin);
  fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
  assert_eq!(outcome.exit, Exit::Success);
  let warning = outcome.stderr.clone().unwrap_or_default();
  assert!(
    warning.starts_with("jev: cannot remove the retired "),
    "{warning}"
  );
  assert!(outcome.stdout.unwrap_or_default().contains("toolu jev"));
}

#[test]
fn a_plugin_root_with_quotes_and_spaces_stays_json_string_data() {
  let sb = Sandbox::new();
  let plugin = install(&sb.dir.path().join("it's a \"plugin\"\nroot"));
  let context = context_of(&session_start(&sb.env("claude"), &plugin), EVENT);
  assert_eq!(context, mandate(&skill_file(&plugin)));
}
