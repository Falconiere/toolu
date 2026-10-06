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
fn opencode_reads_its_own_home_then_xdg() {
  let own = roots(
    Host::Opencode,
    &[("TOOLU_OPENCODE_HOME", "/oc"), ("XDG_CONFIG_HOME", "/x")],
  );
  assert_eq!(own.config_root(), PathBuf::from("/oc"));
  let xdg = roots(Host::Opencode, &[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")]);
  assert_eq!(xdg.config_root(), PathBuf::from("/x/opencode"));
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
  let opencode = roots(
    Host::Opencode,
    &[("TOOLU_PLUGIN_ROOT", "/op"), ("CLAUDE_PLUGIN_ROOT", "/c")],
  );
  assert_eq!(opencode.plugin_root(), Some(PathBuf::from("/op")));
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
    (
      Host::Opencode,
      Some("npx @toolu/plugins remove jev --host opencode --yes"),
    ),
    (Host::Cursor, None),
    (Host::Hermes, None),
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
