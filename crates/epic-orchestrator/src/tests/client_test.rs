use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use super::{exchange_retry, spool_path, with_protocol};
use crate::paths::Paths;

#[test]
fn a_request_keeps_its_fields_and_records_the_protocol() {
  let body = with_protocol(&serde_json::json!({"op": "wait", "max_seconds": 0}), 1);
  assert_eq!(body["op"], "wait");
  assert_eq!(body["protocol"], 1);
  let path = spool_path(std::path::Path::new("/tmp/epic"), "t1").expect("token");
  assert_eq!(path, std::path::PathBuf::from("/tmp/epic/spool/t1.json"));
  assert!(spool_path(std::path::Path::new("/tmp/epic"), "../x").is_err());
}

#[test]
fn a_dropped_connection_is_not_retried() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let listener = UnixListener::bind(paths.socket()).expect("bind");
  std::thread::spawn(move || {
    let _accepted = listener.accept();
  });
  let started = Instant::now();
  let err = exchange_retry(&paths, 1, &serde_json::json!({"op": "status"})).expect_err("closed");
  assert!(started.elapsed() < Duration::from_millis(400), "{err}");
}

#[test]
fn a_newer_client_spools_its_report_and_replaces_an_older_engine() {
  use std::io::{BufRead, BufReader, Write};
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let listener = UnixListener::bind(paths.socket()).expect("bind");
  let stand_in = std::thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept");
    writeln!(
      stream,
      "{}",
      serde_json::json!({"op": "hello", "protocol": 1})
    )
    .expect("hello");
    let mut line = String::new();
    BufReader::new(stream.try_clone().expect("clone"))
      .read_line(&mut line)
      .expect("request");
    writeln!(stream, "{}", serde_json::json!({"ok": true})).expect("ack");
    serde_json::from_str::<serde_json::Value>(&line).expect("json")
  });
  let report = serde_json::json!({"op": "report", "token": "t-36", "key": "a", "phase": "spec"});
  let reply = exchange_retry(&paths, crate::PROTOCOL, &report).expect("exchange");
  assert_eq!(reply["ok"], true);
  let sent = stand_in.join().expect("stand-in");
  assert_eq!(sent, serde_json::json!({"op": "replace", "protocol": 1}));
  let spooled: serde_json::Value = serde_json::from_str(
    &std::fs::read_to_string(spool_path(tmp.path(), "t-36").expect("path")).expect("spool"),
  )
  .expect("json");
  assert_eq!(spooled, report);
}

#[test]
fn a_large_herdr_reply_reads_whole_and_an_endless_line_fails() {
  let (mut ours, mut theirs) = std::os::unix::net::UnixStream::pair().unwrap();
  let line = "x".repeat(200_000);
  let writer = std::thread::spawn(move || {
    std::io::Write::write_all(&mut theirs, format!("{line}\nnext\n").as_bytes()).unwrap();
  });
  assert_eq!(
    super::read_text(&mut ours).map(|text| text.len()),
    Ok(200_000)
  );
  writer.join().unwrap();

  let (mut ours, mut theirs) = std::os::unix::net::UnixStream::pair().unwrap();
  let writer = std::thread::spawn(move || {
    let _ = std::io::Write::write_all(&mut theirs, &vec![b'y'; 70_000]);
  });
  assert_eq!(
    super::read_capped(&mut ours, 1, 65_536),
    Err("reply line exceeds 65536 bytes".to_owned())
  );
  writer.join().unwrap();
}

#[test]
fn a_line_read_leaves_the_next_line_on_the_stream() {
  let (mut ours, mut theirs) = std::os::unix::net::UnixStream::pair().unwrap();
  std::io::Write::write_all(&mut theirs, b"{\"protocol\":2}\nsecond\n").unwrap();
  assert_eq!(
    super::read_line(&mut ours),
    Ok("{\"protocol\":2}".to_owned())
  );
  assert_eq!(super::read_text(&mut ours), Ok("second".to_owned()));
}
