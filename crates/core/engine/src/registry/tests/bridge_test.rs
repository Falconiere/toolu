use std::path::{Path, PathBuf};
use std::time::Duration;

use toolu_protocol::decision::Decision;
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::Wait;
use toolu_runtime::registry::ModuleKind;

use super::runner::Request;
use super::{Launch, Line, failure, parse_lines, spec_for, stops};
use crate::registry::Entry;

fn entry(file: &str) -> Entry {
  Entry {
    file: file.to_owned(),
    path: PathBuf::from(format!("/r/{file}")),
    spec: "x@t".to_owned(),
    name: file.to_owned(),
    kind: ModuleKind::Esm,
  }
}

#[test]
fn only_marked_lines_for_the_expected_modules_count() {
  let (a, b, c) = (entry("a.js"), entry("b.js"), entry("c.js"));
  let modules = [&a, &b, &c];
  let stdout = "noise\ntoolu-bridge:{\"file\":\"a.js\",\"decision\":{\"kind\":\"allow\"}}\n\
                more noise\ntoolu-bridge:{\"file\":\"b.js\",\"error\":\"boom\"}\n\
                toolu-bridge:{\"file\":\"c.js\",\"decision\":{\"kind\":\"nope\"}}\n";
  assert_eq!(
    parse_lines(stdout, &modules),
    [
      ("a.js".to_owned(), Line::Decision(Decision::Allow)),
      ("b.js".to_owned(), Line::Error("boom".to_owned())),
    ]
  );
  let wrong = "toolu-bridge:{\"file\":\"b.js\",\"error\":\"x\"}\n";
  assert_eq!(parse_lines(wrong, &modules), []);
  for broken in [
    "toolu-bridge:not json\n",
    "toolu-bridge:{\"file\":\"a.js\"}\n",
  ] {
    assert_eq!(parse_lines(broken, &modules), [], "{broken}");
  }
}

#[test]
fn a_line_with_a_lone_surrogate_still_reads() {
  let entry = entry("a.js");
  let line = "toolu-bridge:{\"file\":\"a.js\",\"decision\":{\"kind\":\"deny\",\"reason\":\"cut \\ud83d\"}}\n";
  let reason = Text::new("cut \u{fffd}").unwrap();
  assert_eq!(
    parse_lines(line, &[&entry]),
    [("a.js".to_owned(), Line::Decision(Decision::Deny { reason }))],
    "a deny whose reason JavaScript sliced through an emoji still denies"
  );
}

#[test]
fn a_stop_line_and_a_failure_say_what_happened() {
  let deny = Line::Decision(Decision::Deny {
    reason: Text::new("no").unwrap(),
  });
  let block = Line::Decision(Decision::Block {
    reason: Text::new("no").unwrap(),
  });
  assert!(stops(&deny, "deny") && !stops(&deny, "post_block"));
  assert!(stops(&block, "post_block") && !stops(&block, "deny"));
  assert!(!stops(&Line::Error("x".to_owned()), "deny"));
  let second = Duration::from_secs(2);
  assert_eq!(failure(true, 0, second), "timed out after 2000 ms");
  assert_eq!(failure(false, 3, second), "bridge exited 3");
  assert_eq!(failure(false, 0, second), "bridge output unreadable");
}

#[test]
fn a_batch_gets_one_module_deadline_per_module() {
  let env = Env::from_pairs([("PATH", "/bin")]);
  let launch = Launch {
    bun: Path::new("/opt/bun"),
    env: &env,
    cwd: Path::new("/p"),
    module_timeout: Duration::from_secs(3),
  };
  let empty = Ordered::Object(Vec::new());
  let request = Request {
    stop: "deny",
    registry_event: "tool/pre",
    event: &empty,
    ctx: &empty,
  };
  let (a, b) = (entry("a.js"), entry("b.js"));
  let spec = spec_for(&launch, &request, &[&a, &b]);
  assert_eq!(spec.timeout, Duration::from_secs(6));
  assert_eq!(
    spec.argv.get(..3).unwrap(),
    ["/opt/bun", "--no-install", "-e"]
  );
  assert_eq!(
    (spec.wait, spec.cwd.as_deref()),
    (Wait::Streams, Some(Path::new("/p")))
  );
}
