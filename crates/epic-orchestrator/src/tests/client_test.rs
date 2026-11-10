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
fn cleanup_protocol_mismatch_neither_spools_nor_replaces_the_engine() {
  use std::io::{BufRead as _, BufReader};
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let listener = UnixListener::bind(paths.socket()).unwrap();
  let peer = std::thread::spawn(move || {
    let (mut stream, _) = listener.accept().unwrap();
    stream
      .set_read_timeout(Some(Duration::from_secs(1)))
      .unwrap();
    super::write_json(
      &mut stream,
      &serde_json::json!({"op": "hello", "protocol": crate::PROTOCOL - 1}),
    )
    .unwrap();
    let mut request = String::new();
    BufReader::new(stream).read_line(&mut request).unwrap();
    request
  });
  let error = exchange_retry(
    &paths,
    crate::PROTOCOL,
    &serde_json::json!({"op": "cleanup"}),
  )
  .unwrap_err();
  assert!(error.contains("protocol"), "{error}");
  assert!(error.contains("engine --replace"), "{error}");
  assert_eq!(peer.join().unwrap(), "");
  assert!(!paths.spool().exists());
}

#[test]
fn local_only_old_engine_protocol_fails_before_spool_or_replace() {
  use std::io::{BufRead as _, BufReader};
  for request in [
    serde_json::json!({"op":"report", "token":"r-old", "key":"a", "phase":"spec"}),
    serde_json::json!({"op":"notify", "token":"n-old", "key":"a", "kind":"stuck"}),
  ] {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::at(dir.path());
    let listener = UnixListener::bind(paths.socket()).unwrap();
    let peer = std::thread::spawn(move || {
      let (mut stream, _) = listener.accept().unwrap();
      stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
      super::write_json(
        &mut stream,
        &serde_json::json!({"op":"hello", "protocol":2}),
      )
      .unwrap();
      let mut sent = String::new();
      BufReader::new(stream.try_clone().unwrap())
        .read_line(&mut sent)
        .unwrap();
      if !sent.is_empty() {
        super::write_json(&mut stream, &serde_json::json!({"ok":true})).unwrap();
      }
      sent
    });
    let outcome = super::exchange(&paths, 3, &request);
    assert_eq!(
      peer.join().unwrap(),
      "",
      "old engine must receive no mutation"
    );
    assert!(
      !paths.spool().exists(),
      "unsupported protocol must not spool"
    );
    let error = outcome.unwrap_err();
    assert!(error.contains("protocol"), "{error}");
    assert!(error.contains("engine --replace"), "{error}");
  }
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
