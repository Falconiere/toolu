use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;

use super::{Mode, Target, available, check_scope, on_path, resolve, target_of};
use crate::args::Scope;
use crate::support::{Answer, Sandbox};

#[test]
fn installer_targets_are_claude_codex_and_opencode() {
  assert_eq!(target_of(Host::Claude), Ok(Target::Claude));
  assert_eq!(target_of(Host::Codex), Ok(Target::Codex));
  assert_eq!(target_of(Host::Opencode), Ok(Target::Opencode));
  for host in [Host::Cursor, Host::Hermes] {
    let Err(fail) = target_of(host) else {
      panic!("{} was accepted", host.name());
    };
    assert_eq!(fail.exit, Exit::Usage);
    assert!(
      fail.message.contains("claude, codex or opencode"),
      "{}",
      fail.message
    );
  }
}

#[test]
fn detection_reads_executable_files_on_path_in_probe_order() {
  let sb = Sandbox::new();
  assert_eq!(available(&sb.env()), Vec::<Target>::new());
  sb.stub("opencode", "exit 0");
  sb.stub("claude", "exit 0");
  sb.write("bin/codex", "not executable");
  assert!(on_path("claude", &sb.env()));
  assert!(!on_path("codex", &sb.env()));
  assert_eq!(available(&sb.env()), [Target::Claude, Target::Opencode]);
}

#[test]
fn an_explicit_host_wins_and_one_found_host_is_used() {
  let sb = Sandbox::new();
  assert_eq!(
    resolve(Some(Target::Codex), Mode::One, &sb.env(), None),
    Ok(vec![Target::Codex])
  );
  let Err(none) = resolve(None, Mode::One, &sb.env(), None) else {
    panic!("no host resolved to one");
  };
  assert_eq!(none.exit, Exit::Usage);
  assert_eq!(
    none.message,
    "no host found on PATH (probed claude, codex, opencode). Choose one with --host."
  );
  sb.stub("codex", "exit 0");
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), None),
    Ok(vec![Target::Codex])
  );
}

#[test]
fn several_hosts_ask_or_refuse_naming_them() {
  let sb = Sandbox::new();
  sb.stub("claude", "exit 0");
  sb.stub("codex", "exit 0");
  let Err(refused) = resolve(None, Mode::Many, &sb.env(), None) else {
    panic!("ambiguity resolved silently");
  };
  assert_eq!(refused.exit, Exit::Usage);
  assert_eq!(
    refused.message,
    "several hosts found (claude, codex). Choose one with --host."
  );
  let mut both = Answer(vec![0, 1]);
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), Some(&mut both)),
    Ok(vec![Target::Claude, Target::Codex])
  );
  let mut second = Answer(vec![1]);
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), Some(&mut second)),
    Ok(vec![Target::Codex])
  );
  let mut cancel = Answer(Vec::new());
  let cancelled = resolve(None, Mode::One, &sb.env(), Some(&mut cancel));
  assert_eq!(
    cancelled.map_err(|fail| (fail.exit, fail.message)),
    Err((Exit::Failure, "cancelled".to_owned()))
  );
  let mut nothing = Answer(Vec::new());
  let empty = resolve(None, Mode::Many, &sb.env(), Some(&mut nothing));
  assert_eq!(empty.map_err(|fail| fail.exit), Err(Exit::Usage));
}

#[test]
fn scopes_follow_each_host() {
  for scope in [Scope::User, Scope::Project, Scope::Local] {
    assert_eq!(check_scope(Some(scope), Target::Claude), Ok(()));
  }
  assert_eq!(check_scope(Some(Scope::User), Target::Opencode), Ok(()));
  assert_eq!(check_scope(None, Target::Codex), Ok(()));
  let local = check_scope(Some(Scope::Local), Target::Opencode).map_err(|fail| fail.message);
  assert_eq!(
    local,
    Err("--scope local is Claude Code only; OpenCode takes user or project".to_owned())
  );
  let codex =
    check_scope(Some(Scope::User), Target::Codex).map_err(|fail| (fail.exit, fail.message));
  assert_eq!(
    codex,
    Err((
      Exit::Usage,
      "--scope is for Claude Code and OpenCode; codex has no scope concept".to_owned()
    ))
  );
}
