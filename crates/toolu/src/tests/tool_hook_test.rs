use std::io::Read as _;
use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use toolu_engine::DispatchOptions;
use toolu_engine::dispatch::DEFAULT_MODULE_TIMEOUT;
use toolu_engine::gate::Gate;
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::rule::RuleContext;

use super::{Given, Phase, RULES, cwd_error, hook_main, lib_dir, line, tool_hook};

/// A built-in gate with a bug.
struct Panics;

impl Gate for Panics {
  fn name(&self) -> &'static str {
    "panics"
  }

  fn run(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Result<Decision, String> {
    Ok(bug())
  }
}

/// The gate's bug.
fn bug() -> Decision {
  panic!("gate bug")
}

#[test]
fn one_trailing_newline_goes_and_an_empty_stream_is_none() {
  assert_eq!(line(b"{\"a\":1}\n"), Some("{\"a\":1}".to_owned()));
  assert_eq!(line(b"two\n\n"), Some("two\n".to_owned()));
  assert_eq!(line(b"bare"), Some("bare".to_owned()));
  assert_eq!(line(b"\n"), None);
  assert_eq!(line(b""), None);
}

#[test]
fn the_given_payload_reads_back_or_fails_once() {
  let mut text = String::new();
  Given(Ok(std::io::Cursor::new(b"payload".to_vec())))
    .read_to_string(&mut text)
    .unwrap();
  assert_eq!(text, "payload");
  let mut broken = Given(Err(Some(std::io::Error::other("bad bytes"))));
  let mut buf = [0u8; 4];
  assert_eq!(broken.read(&mut buf).unwrap_err().to_string(), "bad bytes");
  assert_eq!(
    broken.read(&mut buf).unwrap_err().to_string(),
    "the payload could not be read"
  );
}

#[test]
fn the_lib_dir_comes_from_the_plugin_root() {
  let roots = Roots::new(
    Env::from_pairs([("CLAUDE_PLUGIN_ROOT", "/env/plugin")]),
    None,
  );
  assert_eq!(
    lib_dir(Some(Path::new("/flag")), &roots),
    Path::new("/flag/hooks/lib")
  );
  assert_eq!(lib_dir(None, &roots), Path::new("/env/plugin/hooks/lib"));
  assert_eq!(
    lib_dir(None, &Roots::new(Env::default(), None)),
    Path::new("")
  );
  assert_eq!(RULES.len(), 0);
}

#[test]
fn an_unreadable_payload_blocks_before_and_after_a_tool() {
  let unreadable = || Err(std::io::Error::other("stream did not contain valid UTF-8"));
  let pre = tool_hook(Phase::Pre, unreadable(), None);
  assert_eq!((pre.exit, pre.stdout), (Exit::Blocked, None));
  assert_eq!(
    pre.stderr.as_deref(),
    Some(
      "blocked: toolu PreToolUse hook failed: the hook payload could not be read: stream did not contain valid UTF-8"
    )
  );
  let post = tool_hook(Phase::Post, unreadable(), None);
  assert_eq!((post.exit, post.stdout), (Exit::Blocked, None));
  assert_eq!(
    post.stderr.as_deref(),
    Some(
      "toolu PostToolUse hook failed: the hook payload could not be read: stream did not contain valid UTF-8"
    )
  );
}

#[test]
fn a_panicking_gate_blocks_before_and_after_a_tool() {
  let env = Env::from_pairs([("HOME", "/nonexistent/toolu-home")]);
  let options = DispatchOptions {
    env: &env,
    cwd: Path::new("/"),
    lib_dir: Path::new("/lib"),
    builtins: &[&Panics],
    rules: RULES,
    selected_specs: None,
    continue_post_blocks: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  };
  let payload = || Ok(r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#.to_owned());
  let pre = hook_main(Phase::Pre, payload(), Host::Claude, Ok(&options));
  assert_eq!((pre.exit, pre.stdout), (Exit::Blocked, None));
  assert_eq!(
    pre.stderr.as_deref(),
    Some("blocked: toolu PreToolUse hook panicked: gate bug")
  );
  let post = hook_main(Phase::Post, payload(), Host::Claude, Ok(&options));
  assert_eq!((post.exit, post.stdout), (Exit::Blocked, None));
  assert_eq!(
    post.stderr.as_deref(),
    Some("toolu PostToolUse hook panicked: gate bug")
  );
}

#[test]
fn a_working_directory_the_os_cannot_give_blocks_before_and_after_a_tool() {
  let gone = std::io::Error::from(std::io::ErrorKind::NotFound);
  let payload = || Ok(r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#.to_owned());
  let pre = hook_main(Phase::Pre, payload(), Host::Claude, Err(cwd_error(&gone)));
  assert_eq!((pre.exit, pre.stdout), (Exit::Blocked, None));
  assert_eq!(
    pre.stderr.as_deref(),
    Some(
      "blocked: toolu PreToolUse hook failed: the working directory could not be read: entity not found"
    )
  );
  let post = hook_main(Phase::Post, payload(), Host::Claude, Err(cwd_error(&gone)));
  assert_eq!((post.exit, post.stdout), (Exit::Blocked, None));
  assert_eq!(
    post.stderr.as_deref(),
    Some(
      "toolu PostToolUse hook failed: the working directory could not be read: entity not found"
    )
  );
}
