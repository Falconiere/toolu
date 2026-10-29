use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use super::{Msg, session};
use crate::PROTOCOL;
use crate::client::{read_json, write_json};

fn session_pair(tx: &mpsc::Sender<Msg>) -> (UnixStream, thread::JoinHandle<()>) {
  let (mut peer, server) = UnixStream::pair().expect("pair");
  let send = tx.clone();
  let handle = thread::spawn(move || {
    session(
      server,
      &send,
      PROTOCOL,
      Instant::now() + Duration::from_secs(5),
    );
  });
  assert_eq!(read_json(&mut peer).expect("hello")["op"], "hello");
  (peer, handle)
}

#[test]
fn idle_and_partial_peers_release_their_sessions() {
  let (tx, _rx) = mpsc::channel();
  let mut peers = Vec::new();
  let mut sessions = Vec::new();
  for partial in [false, true] {
    let (mut peer, session) = session_pair(&tx);
    sessions.push(session);
    if partial {
      peer.write_all(b"{\"op\":").expect("partial request");
    }
    peers.push(peer);
  }
  thread::sleep(Duration::from_secs(6));
  let finished: Vec<_> = sessions
    .iter()
    .map(thread::JoinHandle::is_finished)
    .collect();
  drop(peers);
  for session in sessions {
    session.join().expect("session");
  }
  assert_eq!(finished, [true, true]);
}

#[test]
fn a_byte_trickle_cannot_renew_the_total_request_deadline() {
  let (tx, _rx) = mpsc::channel();
  let (mut peer, session) = session_pair(&tx);
  for _ in 0..12 {
    if peer.write_all(b" ").is_err() {
      break;
    }
    thread::sleep(Duration::from_millis(500));
  }
  let finished = session.is_finished();
  drop(peer);
  session.join().expect("session");
  assert!(finished, "trickling peer retained its server session");
}

#[test]
fn accepted_wait_and_replace_replies_outlive_the_read_deadline() {
  let (tx, rx) = mpsc::channel();
  let mut peers = Vec::new();
  let mut sessions = Vec::new();
  for op in ["wait", "replace"] {
    let (mut peer, session) = session_pair(&tx);
    sessions.push(session);
    write_json(&mut peer, &json!({"op": op, "protocol": PROTOCOL})).expect("request");
    peers.push(peer);
  }
  let replies: Vec<_> = (0..2).map(|_| rx.recv().expect("accepted")).collect();
  thread::sleep(Duration::from_secs(6));
  for message in replies {
    let Msg::Line { request, reply } = message else {
      panic!("request")
    };
    reply.send(json!({"ok": request["op"]})).expect("reply");
  }
  for (mut peer, op) in peers.into_iter().zip(["wait", "replace"]) {
    assert_eq!(read_json(&mut peer).expect("late reply")["ok"], op);
  }
  for session in sessions {
    session.join().expect("session");
  }
}

#[test]
fn malformed_and_oversized_requests_close_without_dispatch() {
  for request in [b"{bad}\n".to_vec(), vec![b' '; 16 * 1024 * 1024 + 1]] {
    let (tx, rx) = mpsc::channel();
    let (mut peer, session) = session_pair(&tx);
    let _sent = peer.write_all(&request);
    session.join().expect("session");
    assert!(rx.try_recv().is_err(), "invalid request was dispatched");
    let mut byte = [0];
    assert_eq!(
      std::io::Read::read(&mut peer, &mut byte).expect("closed"),
      0
    );
  }
}
