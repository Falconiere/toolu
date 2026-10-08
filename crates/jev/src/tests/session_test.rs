use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::session_start;
use crate::hooks::{
  KEY, Sandbox, assert_silent, called, context_of, install, mandate, node, sh, skill_file,
};

const SHIM: &str = "#!/bin/sh\nexec toolu jev \"$@\"\n";
const EVENT: &str = "SessionStart";

fn start(env: &Env, plugin: &Path) -> toolu_runtime::cli::Outcome {
  session_start(env, plugin, Some("{}"))
}

fn user_file(path: &Path, body: &str, mode: u32) {
  fs::create_dir_all(path.parent().unwrap()).unwrap();
  fs::write(path, body).unwrap();
  fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn publishes_the_shim_and_states_the_mandate_on_claude() {
  let sb = Sandbox::new();
  let context = context_of(&start(&sb.env("claude"), &sb.plugin), EVENT);
  let link = sb.claude_dir().join("jev.sh");
  assert_eq!(
    fs::read_link(&link).unwrap(),
    sb.plugin.join("scripts/jev.sh")
  );
  assert_eq!(fs::read_to_string(&link).unwrap(), SHIM);
  assert_eq!(context, mandate("toolu jev", &skill_file(&sb.plugin)));
  assert!(!context.contains(KEY) && !context.to_lowercase().contains("bun"));
}

#[test]
fn publishes_under_codex_home_with_spaces() {
  let sb = Sandbox::new();
  let codex = sb.dir.path().join("codex profile");
  let env = sb.env("codex").with("CODEX_HOME", codex.to_str().unwrap());
  let context = context_of(&start(&env, &sb.plugin), EVENT);
  assert_eq!(
    fs::read_link(codex.join("jev/jev.sh")).unwrap(),
    sb.plugin.join("scripts/jev.sh")
  );
  assert_eq!(context, mandate("toolu jev", &skill_file(&sb.plugin)));
  assert!(!sb.home.join(".claude").exists());
}

#[test]
fn toolu_config_dir_takes_precedence_over_the_codex_root() {
  let sb = Sandbox::new();
  let custom = sb.dir.path().join("custom profile");
  let codex = sb.dir.path().join("codex");
  let env = sb.env("codex").with("CODEX_HOME", codex.to_str().unwrap());
  let env = env.with("TOOLU_CONFIG_DIR", custom.to_str().unwrap());
  context_of(&start(&env, &sb.plugin), EVENT);
  assert!(custom.join("jev/jev.sh").is_symlink());
  assert!(!codex.join("jev").exists());
}

#[test]
fn without_the_hook_key_the_command_environment_is_checked_first() {
  let sb = Sandbox::new();
  let env = Env::from_pairs([("HOME", sb.home.to_str().unwrap())]);
  let context = context_of(&session_start(&env, &sb.plugin, Some("{}")), EVENT);
  assert!(context.starts_with("The Jev hook did not receive TYPESAFE_API_KEY."));
  assert!(context.contains(
    "check whether TYPESAFE_API_KEY is set in the command environment without printing its value"
  ));
  assert!(!context.contains("Jev unavailable (missing:"));
  assert!(context.contains(&mandate("toolu jev", &skill_file(&sb.plugin))));
}

#[test]
fn the_published_shim_runs_toolu_jev_with_the_key_and_no_runtime_on_path() {
  let sb = Sandbox::new();
  let bin = sb.dir.path().join("bin");
  user_file(&bin.join("toolu"), "#!/bin/sh\necho \"toolu $*\"\n", 0o755);
  for host in ["claude", "codex"] {
    let config = sb
      .dir
      .path()
      .join(format!("{host} 'quoted' $(no-command) profile"));
    let env = sb
      .env(host)
      .with("TOOLU_CONFIG_DIR", config.to_str().unwrap());
    let context = context_of(&start(&env, &sb.plugin), EVENT);
    assert_eq!(called(&context), "toolu jev");
    let shim = config
      .join("jev/jev.sh")
      .display()
      .to_string()
      .replace('\'', "'\\''");
    let probe = sh(
      &format!("'{shim}' noul probe -s evidence"),
      &env.with("PATH", bin.to_str().unwrap()),
    );
    assert_eq!(
      (probe.exit_code, probe.stdout.as_str()),
      (0, "toolu jev noul probe -s evidence\n")
    );
  }
}

#[test]
fn paths_with_quotes_spaces_and_newlines_stay_json_string_data() {
  let sb = Sandbox::new();
  let plugin = install(&sb.dir.path().join("plugin \"cache\"\nfolder"));
  let config = sb.dir.path().join("profile \"quoted\"\nfolder");
  let env = sb
    .env("claude")
    .with("TOOLU_CONFIG_DIR", config.to_str().unwrap());
  let context = context_of(&start(&env, &plugin), EVENT);
  assert_eq!(context, mandate("toolu jev", &skill_file(&plugin)));
  assert_eq!(
    fs::read_link(config.join("jev/jev.sh")).unwrap(),
    plugin.join("scripts/jev.sh")
  );
}

#[test]
fn refreshes_a_stale_link_and_is_idempotent() {
  let sb = Sandbox::new();
  let link = sb.claude_dir().join("jev.sh");
  fs::create_dir_all(sb.claude_dir()).unwrap();
  symlink("/nonexistent/old/jev.sh", &link).unwrap();
  context_of(&start(&sb.env("claude"), &sb.plugin), EVENT);
  assert_eq!(
    fs::read_link(&link).unwrap(),
    sb.plugin.join("scripts/jev.sh")
  );
  let inode = fs::symlink_metadata(&link).unwrap().ino();
  context_of(&start(&sb.env("claude"), &sb.plugin), EVENT);
  assert_eq!(fs::symlink_metadata(&link).unwrap().ino(), inode);
}

#[test]
fn a_users_executable_shell_override_keeps_its_own_interpreter() {
  let sb = Sandbox::new();
  let dst = sb.claude_dir().join("jev.sh");
  user_file(&dst, "#!/bin/sh\necho user-override\n", 0o755);
  let context = context_of(&start(&sb.env("claude"), &sb.plugin), EVENT);
  assert!(!dst.is_symlink());
  assert_eq!(
    fs::read_to_string(&dst).unwrap(),
    "#!/bin/sh\necho user-override\n"
  );
  let command = format!("'{}'", dst.display());
  assert_eq!(context, mandate(&command, &skill_file(&sb.plugin)));
  let probe = sh(called(&context), &Env::default());
  assert_eq!(
    (probe.exit_code, probe.stdout.as_str()),
    (0, "user-override\n")
  );
}

#[test]
fn a_users_javascript_override_keeps_its_interpreter_without_a_runtime_on_path() {
  let Some(node) = node() else { return };
  let sb = Sandbox::new();
  let dst = sb.claude_dir().join("jev.sh");
  let body = format!(
    "#!{}\nconsole.log(\"javascript-override\");\n",
    node.display()
  );
  user_file(&dst, &body, 0o755);
  let context = context_of(&start(&sb.env("claude"), &sb.plugin), EVENT);
  assert_eq!(fs::read_to_string(&dst).unwrap(), body);
  assert_eq!(called(&context), format!("'{}'", dst.display()));
  let env = Env::from_pairs([("PATH", "/nonexistent"), ("TYPESAFE_API_KEY", KEY)]);
  let probe = sh(called(&context), &env);
  assert_eq!(
    (probe.exit_code, probe.stdout.as_str()),
    (0, "javascript-override\n")
  );
  assert_eq!(probe.stderr, "");
}

#[test]
fn a_kept_path_with_a_single_quote_is_shell_quoted() {
  let sb = Sandbox::new();
  let config = sb.dir.path().join("it's profile");
  user_file(
    &config.join("jev/jev.sh"),
    "#!/bin/sh\necho quoted\n",
    0o755,
  );
  let env = sb
    .env("claude")
    .with("TOOLU_CONFIG_DIR", config.to_str().unwrap());
  let context = context_of(&start(&env, &sb.plugin), EVENT);
  let dst = config.join("jev/jev.sh").display().to_string();
  assert_eq!(
    called(&context),
    format!("'{}'", dst.replace('\'', "'\\''"))
  );
  assert_eq!(sh(called(&context), &Env::default()).stdout, "quoted\n");
}

#[test]
fn a_missing_shim_source_is_silent_and_publishes_nothing() {
  let sb = Sandbox::new();
  fs::remove_file(sb.plugin.join("scripts/jev.sh")).unwrap();
  assert_silent(&start(&sb.env("claude"), &sb.plugin));
  assert!(!sb.claude_dir().exists());
}

#[test]
fn a_non_executable_user_wrapper_reports_the_installation_problem() {
  let sb = Sandbox::new();
  let dst = sb.claude_dir().join("jev.sh");
  user_file(&dst, "user wrapper\n", 0o644);
  let context = context_of(&start(&sb.env("claude"), &sb.plugin), EVENT);
  assert!(context.starts_with("Jev unavailable: published wrapper is not executable."));
  assert!(context.ends_with("Do not read credentials from .env."));
  assert!(!context.contains("toolu jev"));
  assert_eq!(fs::read_to_string(&dst).unwrap(), "user wrapper\n");
}

#[test]
fn an_uncreatable_config_dir_reports_once_and_emits_no_context() {
  let sb = Sandbox::new();
  let blocker = sb.dir.path().join("blocker");
  fs::write(&blocker, "").unwrap();
  let env = sb
    .env("claude")
    .with("TOOLU_CONFIG_DIR", blocker.to_str().unwrap());
  let outcome = start(&env, &sb.plugin);
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stdout, None);
  let message = format!(
    "jev: cannot create {} — wrapper not published",
    blocker.join("jev").display()
  );
  assert_eq!(outcome.stderr, Some(message));
}

fn path_max() -> usize {
  if cfg!(target_os = "macos") {
    1024
  } else {
    4096
  }
}

/// A config root so deep that `<root>/jev/jev.sh` fits `PATH_MAX` but its temporary link
/// beside it does not, which refuses the link even for a user who may write anywhere.
fn too_deep_root(base: &Path) -> std::path::PathBuf {
  let mut root = base.to_path_buf();
  let target = path_max() - 11 - "/jev/jev.sh".len();
  while root.as_os_str().len() + 201 < target {
    root.push("d".repeat(200));
  }
  let rest = target - root.as_os_str().len() - 1;
  root.push("e".repeat(rest));
  root
}

#[test]
fn a_config_dir_that_refuses_the_link_reports_once_and_emits_no_context() {
  let sb = Sandbox::new();
  let root = too_deep_root(sb.dir.path());
  fs::create_dir_all(root.join("jev")).unwrap();
  let env = sb
    .env("claude")
    .with("TOOLU_CONFIG_DIR", root.to_str().unwrap());
  let outcome = start(&env, &sb.plugin);
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stdout, None);
  assert_eq!(
    outcome.stderr,
    Some(format!(
      "jev: cannot publish {}",
      root.join("jev/jev.sh").display()
    ))
  );
}

#[test]
fn unreadable_or_non_object_stdin_is_a_normal_start_on_opencode_too() {
  let sb = Sandbox::new();
  let expected = mandate("toolu jev", &skill_file(&sb.plugin));
  for stdin in [
    None,
    Some(""),
    Some("bad json"),
    Some("[]"),
    Some("null"),
    Some("42"),
  ] {
    let outcome = session_start(&sb.env("claude"), &sb.plugin, stdin);
    assert_eq!(context_of(&outcome, EVENT), expected);
  }
  let open = sb.env("opencode");
  let skill = "skill({ name: \"jev-jev\" })";
  for stdin in [None, Some("bad json"), Some("[1]")] {
    let outcome = session_start(&open, &sb.plugin, stdin);
    assert_eq!(context_of(&outcome, EVENT), mandate("toolu jev", skill));
  }
}

#[test]
fn a_broken_startup_report_is_an_error_after_the_context() {
  let sb = Sandbox::new();
  let blocker = sb.dir.path().join("report-blocker");
  fs::write(&blocker, "").unwrap();
  let report = blocker.join("report.jsonl");
  let env = sb
    .env("claude")
    .with("TOOLU_STARTUP_REPORT", report.to_str().unwrap());
  let outcome = start(&env, &sb.plugin);
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(
    outcome
      .stdout
      .as_deref()
      .is_some_and(|text| text.contains("additionalContext"))
  );
  assert!(outcome.stderr.is_some());
}
