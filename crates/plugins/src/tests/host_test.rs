use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;

use super::{Mode, Target, available, check_scope, on_path, resolve, target_of};
use crate::args::Scope;
use crate::support::{Answer, Sandbox};

#[test]
fn installer_targets_are_claude_codex_muse_cursor_and_opencode() {
  assert_eq!(target_of(Host::Claude), Ok(Target::Claude));
  assert_eq!(target_of(Host::Codex), Ok(Target::Codex));
  assert_eq!(target_of(Host::Muse), Ok(Target::Muse));
  assert_eq!(target_of(Host::Cursor), Ok(Target::Cursor));
  assert_eq!(target_of(Host::Opencode), Ok(Target::Opencode));
  let Err(fail) = target_of(Host::Hermes) else {
    panic!("hermes was accepted");
  };
  assert_eq!(fail.exit, Exit::Usage);
  assert!(
    fail
      .message
      .contains("claude, codex, muse, cursor or opencode"),
    "{}",
    fail.message
  );
}

#[test]
fn detection_reads_executable_files_on_path_in_probe_order() {
  let sb = Sandbox::new();
  assert_eq!(available(&sb.env()), Vec::<Target>::new());
  sb.stub("claude", "exit 0");
  sb.write("bin/codex", "not executable");
  assert!(on_path("claude", &sb.env()));
  assert!(!on_path("codex", &sb.env()));
  assert_eq!(available(&sb.env()), [Target::Claude]);
  sb.stub("codex", "exit 0");
  assert_eq!(available(&sb.env()), [Target::Claude, Target::Codex]);
  sb.stub("muse", "exit 0");
  assert_eq!(
    available(&sb.env()),
    [Target::Claude, Target::Codex, Target::Muse]
  );
}

#[test]
fn muse_alone_on_path_is_used_without_asking() {
  let sb = Sandbox::new();
  sb.stub("muse", "exit 0");
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), None),
    Ok(vec![Target::Muse])
  );
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), None),
    Ok(vec![Target::Muse])
  );
}

#[test]
fn three_hosts_refuse_without_a_terminal_and_offer_muse_last() {
  let sb = Sandbox::new();
  for bin in ["claude", "codex", "muse"] {
    sb.stub(bin, "exit 0");
  }
  let Err(refused) = resolve(None, Mode::One, &sb.env(), None) else {
    panic!("ambiguity resolved silently");
  };
  assert_eq!(
    (refused.exit, refused.message.as_str()),
    (
      Exit::Usage,
      "several hosts found (claude, codex, muse). Choose one with --host."
    )
  );
  let mut third = Answer(vec![2]);
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), Some(&mut third)),
    Ok(vec![Target::Muse])
  );
  assert_eq!(
    resolve(Some(Target::Muse), Mode::One, &sb.env(), None),
    Ok(vec![Target::Muse])
  );
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
    "no host found on PATH (probed claude, codex, muse). Choose one with --host."
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
  assert_eq!(check_scope(None, Target::Codex), Ok(()));
  let codex =
    check_scope(Some(Scope::User), Target::Codex).map_err(|fail| (fail.exit, fail.message));
  assert_eq!(
    codex,
    Err((
      Exit::Usage,
      "--scope is for Claude Code; codex has no scope concept".to_owned()
    ))
  );
  assert_eq!(check_scope(None, Target::Muse), Ok(()));
  let muse = check_scope(Some(Scope::User), Target::Muse).map_err(|fail| (fail.exit, fail.message));
  assert_eq!(
    muse,
    Err((
      Exit::Usage,
      "--scope is for Claude Code; muse has no scope concept".to_owned()
    ))
  );
}

#[test]
fn only_hosts_whose_removal_reaches_only_toolu_take_retired_names() {
  let hosts = [
    Target::Claude,
    Target::Codex,
    Target::Cursor,
    Target::Opencode,
    Target::Muse,
  ];
  assert_eq!(
    hosts.map(Target::removes_only_toolu),
    [true, true, true, true, false]
  );
}

/// Answers with `pick` and keeps the options it was shown.
struct Seen {
  options: Vec<crate::prompt::Choice>,
  pick: Vec<usize>,
}

impl crate::prompt::Ask for Seen {
  fn many(
    &mut self,
    _message: &str,
    options: &[crate::prompt::Choice],
  ) -> crate::fail::Res<Vec<usize>> {
    self.options = options.to_vec();
    Ok(self.pick.clone())
  }

  fn one(&mut self, _message: &str, options: &[crate::prompt::Choice]) -> crate::fail::Res<usize> {
    self.options = options.to_vec();
    Ok(self.pick.first().copied().unwrap_or_default())
  }
}

fn labels(seen: &Seen) -> Vec<(&str, Option<&str>)> {
  seen
    .options
    .iter()
    .map(|(name, hint)| (name.as_str(), hint.as_deref()))
    .collect()
}

#[test]
fn a_terminal_install_offers_every_detected_host_with_cursor_by_its_config() {
  let sb = Sandbox::new();
  sb.stub("claude", "exit 0");
  sb.stub("codex", "exit 0");
  std::fs::create_dir_all(sb.path("home/.cursor")).expect("cursor config");
  let mut seen = Seen {
    options: Vec::new(),
    pick: vec![0, 2],
  };
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), Some(&mut seen)),
    Ok(vec![Target::Claude, Target::Cursor])
  );
  assert_eq!(
    labels(&seen),
    [
      ("claude", Some("Claude Code")),
      ("codex", Some("Codex")),
      ("cursor", Some("Cursor"))
    ]
  );
  let Err(fail) = resolve(None, Mode::Many, &sb.env(), None) else {
    panic!("a run without a terminal resolved two PATH hosts");
  };
  assert_eq!(fail.exit, Exit::Usage);
  assert!(fail.message.contains("(claude, codex)"), "{}", fail.message);
}

#[test]
fn cursor_agent_on_path_counts_as_cursor_and_a_lone_path_host_is_still_silent() {
  let sb = Sandbox::new();
  sb.stub("claude", "exit 0");
  sb.stub("cursor-agent", "exit 0");
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), None),
    Ok(vec![Target::Claude])
  );
  let mut seen = Seen {
    options: Vec::new(),
    pick: vec![1],
  };
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), Some(&mut seen)),
    Ok(vec![Target::Cursor])
  );
}

#[test]
fn list_and_remove_offer_cursor_only_when_it_holds_toolu_plugins() {
  let sb = Sandbox::new();
  sb.stub("claude", "exit 0");
  std::fs::create_dir_all(sb.path("home/.cursor/plugins/local")).expect("cursor store");
  let mut seen = Seen {
    options: Vec::new(),
    pick: vec![0],
  };
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), Some(&mut seen)),
    Ok(vec![Target::Claude])
  );
  assert!(seen.options.is_empty(), "a lone host was asked about");
  sb.write(
    &format!(
      "home/.cursor/plugins/local/jev/{}",
      toolu_runtime::host::cursor::MARKER
    ),
    "",
  );
  let mut asked = Seen {
    options: Vec::new(),
    pick: vec![1],
  };
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), Some(&mut asked)),
    Ok(vec![Target::Cursor])
  );
  assert_eq!(
    labels(&asked),
    [("claude", Some("Claude Code")), ("cursor", Some("Cursor"))]
  );
}
