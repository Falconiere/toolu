use std::time::Duration;

use toolu_protocol::exit::Exit;

use super::Local;
use crate::args::Scope;
use crate::dispatch::Context;
use crate::host::{Mode, Target, check_scope, resolve};
use crate::prompt::{Ask, Choice};
use crate::support::Sandbox;

/// Answers with `pick` and keeps the options it was shown.
struct Seen {
  options: Vec<Choice>,
  pick: usize,
}

impl Ask for Seen {
  fn many(&mut self, _message: &str, options: &[Choice]) -> crate::fail::Res<Vec<usize>> {
    self.options = options.to_vec();
    Ok(vec![self.pick])
  }

  fn one(&mut self, _message: &str, options: &[Choice]) -> crate::fail::Res<usize> {
    self.options = options.to_vec();
    Ok(self.pick)
  }
}

fn names(seen: &Seen) -> Vec<&str> {
  seen.options.iter().map(|(name, _)| name.as_str()).collect()
}

pub(crate) fn context(sb: &Sandbox, version: &'static str, origin: &str) -> Context<'static> {
  Context {
    env: sb.env(),
    host: None,
    ask: None,
    timeout: Duration::from_secs(60),
    version,
    origin: origin.to_owned(),
    style: toolu_runtime::term::Style::Plain,
    started: std::time::Instant::now(),
    progress: None,
  }
}

#[test]
fn the_checkout_is_the_hosts_own_and_follows_the_binary_tag() {
  let sb = Sandbox::new();
  let cx = context(&sb, "9.9.8", "file:///nowhere");
  let cursor = Local::new(&cx, Target::Cursor).unwrap();
  assert_eq!(cursor.checkout.tag, "v9.9.8");
  assert!(cursor.checkout.dir.ends_with("toolu/cursor-marketplace"));
  assert!(cursor.store.dir.ends_with(".cursor/plugins/local"));
  assert_eq!(cursor.host(), "cursor");
  let opencode = Local::new(&cx, Target::Opencode).unwrap();
  assert!(
    opencode
      .checkout
      .dir
      .ends_with("toolu/opencode-marketplace")
  );
  assert!(opencode.store.dir.ends_with(".config/opencode/plugins"));
  assert_eq!(opencode.host(), "opencode");
}

#[test]
fn a_copy_is_described_as_the_cp_it_stands_for() {
  let sb = Sandbox::new();
  let cursor = Local::new(&context(&sb, "9.9.8", "file:///nowhere"), Target::Cursor).unwrap();
  let argv = cursor.copy_argv("toolu");
  assert_eq!(&argv[..2], ["cp", "-R"]);
  assert!(
    argv[2].ends_with("cursor-marketplace/plugins/toolu"),
    "{argv:?}"
  );
  assert!(argv[3].ends_with(".cursor/plugins/local/toolu"), "{argv:?}");
}

#[test]
fn opencode_is_offered_by_its_config_and_listed_only_when_it_holds_toolu_plugins() {
  let sb = Sandbox::new();
  sb.stub("claude", "exit 0");
  std::fs::create_dir_all(sb.path("home/.config/opencode")).expect("opencode config");
  let mut seen = Seen {
    options: Vec::new(),
    pick: 1,
  };
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), Some(&mut seen)),
    Ok(vec![Target::Opencode])
  );
  assert_eq!(names(&seen), ["claude", "opencode"]);
  assert_eq!(
    resolve(None, Mode::Many, &sb.env(), None),
    Ok(vec![Target::Claude])
  );
  let mut lone = Seen {
    options: Vec::new(),
    pick: 0,
  };
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), Some(&mut lone)),
    Ok(vec![Target::Claude])
  );
  assert!(lone.options.is_empty(), "a lone host was asked about");
  let marker = toolu_runtime::host::cursor::MARKER;
  sb.write(&format!("home/.config/opencode/plugins/jev/{marker}"), "");
  let mut asked = Seen {
    options: Vec::new(),
    pick: 1,
  };
  assert_eq!(
    resolve(None, Mode::One, &sb.env(), Some(&mut asked)),
    Ok(vec![Target::Opencode])
  );
  assert_eq!(
    check_scope(Some(Scope::User), Target::Opencode).map_err(|fail| fail.exit),
    Err(Exit::Usage)
  );
}
