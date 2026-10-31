use super::cursor;
use crate::journal::{self, Record};
use crate::model::World;
use crate::paths::Paths;
use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};
use toolu_runtime::config::secrets::Secrets;

#[test]
fn status_stream_reconnect_header_wins_over_query_cursor() {
  let parsed = cursor("since=4&limit=20", Some("7")).expect("cursor");
  assert_eq!(parsed.since, 7);
  assert_eq!(parsed.limit, 20);
  assert!(cursor("limit=501", None).is_err());
  assert!(cursor("since=bad", None).is_err());
}

#[test]
fn poisoned_stream_world_sends_fixed_error() {
  let root = tempfile::tempdir().expect("root");
  let paths = Paths::at(root.path());
  let secrets = Secrets::none();
  let world = Arc::new(Mutex::new(World::new(0)));
  let poisoned = Arc::clone(&world);
  let _poisoned = std::thread::spawn(move || {
    let _guard = poisoned.lock().expect("world lock");
    panic!("poison world");
  })
  .join();
  let changed = Arc::new(Condvar::new());
  let stop = Arc::new(AtomicBool::new(false));
  let feed = super::Feed {
    paths: &paths,
    secrets: &secrets,
    world: &world,
    changed: &changed,
    stop: &stop,
    allowed: &|| true,
  };
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let mut client = TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
  let (mut server, _) = listener.accept().expect("accept");
  assert!(!super::wait_for_change(&mut server, &feed));
  drop(server);
  let mut body = String::new();
  client.read_to_string(&mut body).expect("error frame");
  assert_eq!(
    body,
    "event: error\ndata: {\"schema\":1,\"error\":\"status_unavailable\"}\n\n"
  );
}

fn plant_days(journal: &std::path::Path, lock: &std::path::Path, days: u64) -> u64 {
  let start = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  let mut last = 0;
  for day in 0..days {
    let now = start + Duration::from_secs(day * 86_400);
    last = journal::append(
      journal,
      lock,
      &Record::new("action", "merge-intent", "k", "t", "n"),
      now,
    )
    .expect("append")
    .seq;
  }
  last
}

fn spawn_serve(
  root: PathBuf,
  query: &str,
  bytes: Option<Arc<AtomicU64>>,
) -> (TcpStream, Arc<AtomicBool>, Arc<Condvar>) {
  let stop = Arc::new(AtomicBool::new(false));
  let changed = Arc::new(Condvar::new());
  let (stop_thread, changed_thread) = (Arc::clone(&stop), Arc::clone(&changed));
  let query = query.to_owned();
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let client = TcpStream::connect(listener.local_addr().expect("address")).expect("client");
  let (mut server, _) = listener.accept().expect("accept");
  std::thread::spawn(move || {
    let paths = Paths::at(root);
    let secrets = Secrets::none();
    let world = Arc::new(Mutex::new(World::new(0)));
    crate::journal::index::SINK.with(|slot| *slot.borrow_mut() = bytes);
    let feed = super::Feed {
      paths: &paths,
      secrets: &secrets,
      world: &world,
      changed: &changed_thread,
      stop: &stop_thread,
      allowed: &|| true,
    };
    super::serve(&mut server, cursor(&query, None).expect("cursor"), &feed);
  });
  (client, stop, changed)
}

fn read_for(client: &mut TcpStream, wait: Duration) -> String {
  client
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("timeout");
  let mut buf = Vec::new();
  let mut chunk = [0u8; 4096];
  let started = Instant::now();
  while started.elapsed() < wait {
    match client.read(&mut chunk) {
      Ok(0) => break,
      Ok(n) => buf.extend_from_slice(&chunk[..n]),
      Err(err)
        if err.kind() == std::io::ErrorKind::WouldBlock
          || err.kind() == std::io::ErrorKind::TimedOut => {}
      Err(_err) => break,
    }
  }
  String::from_utf8_lossy(&buf).into_owned()
}

fn stop(flag: &AtomicBool, changed: &Condvar) {
  flag.store(true, Ordering::Relaxed);
  changed.notify_all();
}

#[test]
fn idle_stream_skips_retained_jsonl_then_delivers_an_external_append() {
  let root = tempfile::tempdir().expect("root");
  let paths = Paths::at(root.path());
  let journal = paths.journal_dir();
  let lock = paths.journal_lock();
  let last = plant_days(&journal, &lock, 90);
  let bytes = Arc::new(AtomicU64::new(0));
  let (mut client, flag, changed) = spawn_serve(
    root.path().to_path_buf(),
    "since=0&limit=500",
    Some(Arc::clone(&bytes)),
  );
  let caught = read_for(&mut client, Duration::from_secs(2));
  assert!(caught.contains(&format!("id: {last}\n")), "{caught}");
  bytes.store(0, Ordering::Relaxed);
  std::thread::sleep(Duration::from_secs(3));
  let idle = bytes.swap(0, Ordering::Relaxed);
  assert_eq!(idle, 0, "idle jsonl bytes {idle}");
  let child = std::process::Command::new(std::env::current_exe().expect("exe"))
    .arg("--exact")
    .arg("journal::index::tests::external_journal_writer")
    .env("TOOLU_JOURNAL_DIR", &journal)
    .env("TOOLU_JOURNAL_LOCK", &lock)
    .env("TOOLU_JOURNAL_NOTE", "external")
    .env("TOOLU_JOURNAL_COUNT", "1")
    .env("TOOLU_JOURNAL_SECS", "1776896000")
    .output()
    .expect("writer");
  assert!(
    child.status.success(),
    "{}",
    String::from_utf8_lossy(&child.stderr)
  );
  let body = read_for(&mut client, Duration::from_secs(5));
  assert!(body.contains(&format!("id: {}\n", last + 1)), "{body}");
  stop(&flag, &changed);
}

#[test]
fn retained_gap_emits_reset() {
  let root = tempfile::tempdir().expect("root");
  let paths = Paths::at(root.path());
  let journal = paths.journal_dir();
  let start = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  plant_days(&journal, &paths.journal_lock(), 90);
  journal::retain(&journal, 1, start + Duration::from_hours(89 * 24)).expect("retain");
  let (mut client, flag, changed) =
    spawn_serve(root.path().to_path_buf(), "since=0&limit=10", None);
  let body = read_for(&mut client, Duration::from_secs(2));
  assert!(body.starts_with("event: reset\n"), "{body}");
  assert!(body.contains("\"oldest_seq\":89"), "{body}");
  stop(&flag, &changed);
}
