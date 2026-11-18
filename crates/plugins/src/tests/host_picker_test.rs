use toolu_protocol::exit::Exit;

use super::{Mode, Target, available, resolve};
use crate::support::Sandbox;

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
fn grok_on_path_is_offered_and_used_when_it_is_the_only_host() {
  let sb = Sandbox::new();
  sb.stub("grok", "exit 0");
  assert_eq!(available(&sb.env()), [Target::Grok]);
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), None),
    Ok(vec![Target::Grok])
  );
  sb.stub("claude", "exit 0");
  let mut seen = Seen {
    options: Vec::new(),
    pick: vec![1],
  };
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), Some(&mut seen)),
    Ok(vec![Target::Grok])
  );
  assert_eq!(
    labels(&seen),
    [
      ("claude", Some("Claude Code")),
      ("grok", Some("Grok Build"))
    ]
  );
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
