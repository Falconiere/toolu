use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;

use super::{CallerError, Roots};
use crate::env::Env;

fn roots(host: Host, pairs: &[(&str, &str)]) -> Roots {
  Roots::new(Env::from_pairs(pairs.iter().copied()), Some(host))
}

#[test]
fn a_given_host_is_kept_and_detection_warns_once() {
  let given = roots(Host::Hermes, &[("TOOLU_HOST_OVERRIDE", "bogus")]);
  assert_eq!((given.host(), given.warning()), (Host::Hermes, None));
  let env = Env::from_pairs([("TOOLU_HOST_OVERRIDE", "bogus"), ("PLUGIN_ROOT", "/p")]);
  let detected = Roots::new(env.clone(), None);
  assert_eq!(detected.host(), Host::Codex);
  assert_eq!(detected.env(), &env);
  let warning = "toolu-host: invalid TOOLU_HOST_OVERRIDE 'bogus' (using environment detection)";
  assert_eq!(detected.warning(), Some(warning));
}

#[test]
fn cursor_reads_its_project_and_plugin_variables() {
  let cursor = roots(
    Host::Cursor,
    &[
      ("CURSOR_PROJECT_DIR", "/repo"),
      ("CURSOR_PLUGIN_ROOT", "/cp"),
    ],
  );
  assert_eq!(cursor.project_root(None), Some(PathBuf::from("/repo")));
  assert_eq!(cursor.plugin_root(), Some(PathBuf::from("/cp")));
  let hermes = roots(Host::Hermes, &[("PLUGIN_ROOT", "/p")]);
  assert_eq!(hermes.plugin_root(), None);
}

#[test]
fn without_a_cwd_the_process_directory_finds_the_repository() {
  let path = std::env::var("PATH").unwrap();
  let claude = roots(Host::Codex, &[("PATH", path.as_str())]);
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
  assert_eq!(
    claude.project_root(None),
    Some(std::fs::canonicalize(repo).unwrap())
  );
}

#[test]
fn uninstall_commands_exist_where_a_host_can_remove_a_plugin() {
  let expect = [
    (Host::Claude, Some("claude plugin uninstall jev@toolu")),
    (Host::Codex, Some("codex plugin remove jev@toolu")),
    (Host::Cursor, None),
    (Host::Hermes, None),
    (Host::Muse, Some("muse plugins remove jev")),
  ];
  for (host, command) in expect {
    let command = command.map(str::to_owned);
    assert_eq!(
      roots(host, &[]).plugin_uninstall_command("jev"),
      Ok(command)
    );
  }
  let error = CallerError("plugin_uninstall_command: name must be non-empty".to_owned());
  assert_eq!(
    roots(Host::Claude, &[]).plugin_uninstall_command(""),
    Err(error)
  );
}

#[test]
fn muse_config_root_uses_its_dir_then_xdg_then_home() {
  let custom = roots(Host::Muse, &[("MUSE_CONFIG_DIR", "/muse")]);
  assert_eq!(custom.config_root(), PathBuf::from("/muse"));
  let xdg = roots(
    Host::Muse,
    &[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/xdg")],
  );
  assert_eq!(xdg.config_root(), PathBuf::from("/xdg/muse"));
  let home = roots(Host::Muse, &[("HOME", "/home/u")]);
  assert_eq!(home.config_root(), PathBuf::from("/home/u/.config/muse"));
  let toolu = roots(
    Host::Muse,
    &[("MUSE_CONFIG_DIR", "/muse"), ("TOOLU_CONFIG_DIR", "/toolu")],
  );
  assert_eq!(toolu.config_root(), PathBuf::from("/toolu"));
}

#[test]
fn muse_project_uses_agents_dir_and_git_toplevel() {
  let muse = roots(Host::Muse, &[]);
  assert_eq!(muse.project_dirname(), ".agents");
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
  let repo = std::fs::canonicalize(repo).unwrap();
  assert_eq!(
    muse.project_root(Some(&repo.join("crates/core/runtime"))),
    Some(repo.clone())
  );
  assert_eq!(
    muse.project_config_path(Some(&repo.join("crates/core/runtime"))),
    Some(repo.join(".agents/toolu.config.json"))
  );
  let override_root = roots(Host::Muse, &[("TOOLU_PROJECT_DIR", "/other")]);
  assert_eq!(
    override_root.project_root(None),
    Some(PathBuf::from("/other"))
  );
}

#[test]
fn muse_state_is_separate_from_read_only_project_config() {
  let muse = roots(Host::Muse, &[("TOOLU_PROJECT_DIR", "/repo")]);
  assert_eq!(
    muse.project_config_path(None),
    Some(PathBuf::from("/repo/.agents/toolu.config.json"))
  );
  assert_eq!(
    muse.project_state_root(None, None),
    Some(PathBuf::from("/repo/.toolu/tmp/muse"))
  );
  assert_eq!(
    muse.project_state_dir("push-review", None, Some(Path::new("/worktree"))),
    Ok(Some(PathBuf::from("/worktree/.toolu/tmp/muse/push-review")))
  );
  assert_eq!(
    roots(Host::Muse, &[]).project_state_root(Some(Path::new("/nonexistent")), None),
    None
  );
}

#[test]
fn explicit_project_dirname_still_overrides_muse_config_and_state() {
  let muse = roots(
    Host::Muse,
    &[
      ("TOOLU_PROJECT_DIR", "/repo"),
      ("TOOLU_PROJECT_CONFIG_DIRNAME", ".custom"),
    ],
  );
  assert_eq!(
    muse.project_config_path(None),
    Some(PathBuf::from("/repo/.custom/toolu.config.json"))
  );
  assert_eq!(
    muse.project_state_root(None, Some(Path::new("/worktree"))),
    Some(PathBuf::from("/worktree/.custom/tmp"))
  );
}

#[test]
fn muse_plugin_paths_and_commands_prefer_its_native_variables() {
  let muse = roots(
    Host::Muse,
    &[
      ("MUSE_PLUGIN_ROOT", "/muse/root"),
      ("MUSE_PLUGIN_DATA_DIR", "/muse/data"),
      ("CLAUDE_PLUGIN_ROOT", "/claude/root"),
      ("CLAUDE_PLUGIN_DATA", "/claude/data"),
      ("PLUGIN_ROOT", "/codex/root"),
      ("PLUGIN_DATA", "/codex/data"),
    ],
  );
  assert_eq!(muse.plugin_root(), Some(PathBuf::from("/muse/root")));
  assert_eq!(muse.plugin_data(), Some(PathBuf::from("/muse/data")));
  assert_eq!(
    muse.invocation("toolu", "setup"),
    Ok("/toolu:setup".to_owned())
  );
  assert_eq!(
    muse.plugin_install_command("/plugins/toolu"),
    Ok(Some("muse plugins install /plugins/toolu".to_owned()))
  );
  let fallback = roots(
    Host::Muse,
    &[
      ("CLAUDE_PLUGIN_ROOT", "/claude/root"),
      ("CLAUDE_PLUGIN_DATA", "/claude/data"),
    ],
  );
  assert_eq!(fallback.plugin_root(), Some(PathBuf::from("/claude/root")));
  assert_eq!(fallback.plugin_data(), Some(PathBuf::from("/claude/data")));
}
