use std::io::Read as _;
use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Given, Phase, RULES, lib_dir, line, tool_hook};

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
fn an_unreadable_payload_blocks_before_a_tool() {
  let out = tool_hook(
    Phase::Pre,
    Err(std::io::Error::other("stream did not contain valid UTF-8")),
    None,
  );
  assert_eq!(out.exit, Exit::Blocked);
  assert_eq!(out.stdout, None);
  let stderr = out.stderr.unwrap_or_default();
  assert!(
    stderr.starts_with("blocked: toolu PreToolUse hook failed: the hook payload could not be read"),
    "{stderr}"
  );
}
