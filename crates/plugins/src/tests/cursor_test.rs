use std::time::Duration;

use super::Cursor;
use crate::dispatch::Context;
use crate::support::Sandbox;

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
fn the_checkout_is_cursors_own_and_follows_the_binary_tag() {
  let sb = Sandbox::new();
  let cursor = Cursor::new(&context(&sb, "9.9.8", "file:///nowhere")).unwrap();
  assert_eq!(cursor.checkout.tag, "v9.9.8");
  assert!(cursor.checkout.dir.ends_with("toolu/cursor-marketplace"));
  assert!(cursor.store.dir.ends_with(".cursor/plugins/local"));
}

#[test]
fn a_copy_is_described_as_the_cp_it_stands_for() {
  let sb = Sandbox::new();
  let cursor = Cursor::new(&context(&sb, "9.9.8", "file:///nowhere")).unwrap();
  let argv = cursor.copy_argv("toolu");
  assert_eq!(&argv[..2], ["cp", "-R"]);
  assert!(
    argv[2].ends_with("cursor-marketplace/plugins/toolu"),
    "{argv:?}"
  );
  assert!(argv[3].ends_with(".cursor/plugins/local/toolu"), "{argv:?}");
}
