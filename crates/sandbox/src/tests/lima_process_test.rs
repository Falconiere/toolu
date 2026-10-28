use std::time::Duration;

use toolu_protocol::exit::Exit;

use super::{logged, query, words};

fn argv(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn query_returns_stdout_or_names_the_command_and_its_stderr() {
  assert_eq!(query(&argv(&["sh", "-c", "echo out"])).unwrap(), "out\n");
  let err = query(&argv(&["sh", "-c", "echo bad >&2; exit 3"])).unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert_eq!(err.message, "`sh -c echo bad >&2; exit 3` failed: bad");
  let quiet = query(&argv(&["false"])).unwrap_err();
  assert_eq!(quiet.message, "`false` failed: no message");
  let missing = query(&argv(&["toolu-no-such-limactl"])).unwrap_err();
  assert_eq!(missing.exit, Exit::Unavailable);
}

#[test]
fn logged_writes_the_output_to_the_log_and_names_it_on_failure() {
  let dir = tempfile::tempdir().unwrap();
  let log = dir.path().join("step.log");
  let ok = argv(&["sh", "-c", "echo one; echo two >&2"]);
  logged(&ok, dir.path(), &log, Duration::from_secs(30)).unwrap();
  assert_eq!(std::fs::read_to_string(&log).unwrap(), "one\ntwo\n");
  let bad = argv(&["sh", "-c", "exit 4"]);
  let err = logged(&bad, dir.path(), &log, Duration::from_secs(30)).unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert_eq!(
    err.message,
    format!("`sh -c exit 4` exited 4; see {}", log.display())
  );
  let slow = argv(&["sleep", "5"]);
  let late = logged(&slow, dir.path(), &log, Duration::from_millis(200)).unwrap_err();
  assert!(
    late.message.starts_with("`sleep 5` timed out; see "),
    "{}",
    late.message
  );
  let gone = logged(
    &argv(&["toolu-no-such-limactl"]),
    dir.path(),
    &log,
    Duration::from_secs(5),
  );
  assert_eq!(gone.unwrap_err().exit, Exit::Unavailable);
}

#[test]
fn a_message_stops_before_a_multi_line_script() {
  assert_eq!(
    words(&argv(&[
      "limactl", "shell", "vm", "--", "bash", "-c", "a\nb", "x"
    ])),
    "limactl shell vm -- bash -c"
  );
}
