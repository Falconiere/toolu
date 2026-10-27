use std::net::SocketAddr;
use std::os::unix::net::UnixListener;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::tests::{TOKEN, get, serve, world_with};
use super::*;
use crate::fleet_rig::{config, secrets_of};
use crate::http::Running;
use crate::model::Issue;

fn stalled_socket(listener: UnixListener, count: Arc<AtomicUsize>) -> std::thread::JoinHandle<()> {
  std::thread::spawn(move || {
    let mut held = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
      match listener.accept() {
        Ok((stream, _)) => {
          count.fetch_add(1, Ordering::Relaxed);
          held.push(stream);
        }
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
          std::thread::sleep(Duration::from_millis(5));
        }
        Err(err) => panic!("herdr accept: {err}"),
      }
    }
  })
}

fn nine_workers() -> World {
  let mut world = world_with("worker0", "working");
  for index in 1..9 {
    let issue = Issue::blank(&format!("worker{index}"), "epic", "/tmp/epic", 0);
    world.issues.insert(issue.key.clone(), issue);
  }
  world
}

fn nine_reads(
  addr: SocketAddr,
  same_key: bool,
) -> Vec<std::thread::JoinHandle<(u16, serde_json::Value, usize)>> {
  let barrier = Arc::new(Barrier::new(10));
  let requests = (0..9)
    .map(|index| {
      let barrier = Arc::clone(&barrier);
      std::thread::spawn(move || {
        barrier.wait();
        let key = if same_key {
          "worker".to_owned()
        } else {
          format!("worker{index}")
        };
        get(addr, &format!("/api/v1/issues/{key}/screen"), Some(TOKEN))
      })
    })
    .collect();
  barrier.wait();
  requests
}

fn stalled_screen(
  world: World,
) -> (
  Running,
  Arc<AtomicBool>,
  tempfile::TempDir,
  tempfile::TempDir,
  Arc<AtomicUsize>,
  std::thread::JoinHandle<()>,
) {
  let root = tempfile::tempdir().expect("root");
  let socket = root.path().join("herdr.sock");
  let listener = UnixListener::bind(&socket).expect("herdr socket");
  listener.set_nonblocking(true).expect("nonblocking");
  let accepted = Arc::new(AtomicUsize::new(0));
  let stalled = stalled_socket(listener, Arc::clone(&accepted));
  let secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let mut config = config(0, Vec::new(), secrets);
  config.herdr = Herdr::Socket(socket);
  let (running, stop, server_root) = serve(config, world);
  (running, stop, root, server_root, accepted, stalled)
}

#[test]
fn nine_overlapping_screen_reads_hit_the_shared_limit_without_blocking_overview() {
  let (running, stop, _socket_root, _server_root, accepted, stalled) =
    stalled_screen(nine_workers());
  let requests = nine_reads(running.addr, false);
  std::thread::sleep(Duration::from_millis(200));
  let started = Instant::now();
  let (overview, _) = crate::fleet_rig::get(running.addr, "/api/v1/overview");
  assert_eq!(overview, 200);
  assert!(started.elapsed() < Duration::from_millis(500));
  let answers: Vec<_> = requests
    .into_iter()
    .map(|request| request.join().expect("screen request"))
    .collect();
  assert!(
    answers
      .iter()
      .any(|(status, body, _)| *status == 503 && body["error"] == "herdr_busy"),
    "{answers:?}"
  );
  assert!(
    accepted.load(Ordering::Relaxed) >= 2,
    "separate keys serialized"
  );
  stalled.join().expect("stalled socket");
  running.stop(&stop).expect("listener thread");
}

#[test]
fn same_key_readers_share_one_call_without_using_extra_permits() {
  let (running, stop, _socket_root, _server_root, accepted, stalled) =
    stalled_screen(world_with("worker", "working"));
  let requests = nine_reads(running.addr, true);
  for request in requests {
    let (status, body, _) = request.join().expect("screen request");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["available"], false);
  }
  assert_eq!(accepted.load(Ordering::Relaxed), 1);
  stalled.join().expect("stalled socket");
  running.stop(&stop).expect("listener thread");
}
