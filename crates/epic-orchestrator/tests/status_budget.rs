//! Real Linux status-listener budget under 50 polling clients and eight SSE readers.

#![cfg(target_os = "linux")]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

const CLIENTS: usize = 50;
const REQUESTS: usize = 40;
const STREAMS: usize = 8;

struct Server {
  child: Child,
  addr: SocketAddr,
  _root: tempfile::TempDir,
  streams: Vec<TcpStream>,
}

impl Server {
  fn start() -> Self {
    let root = tempfile::tempdir().expect("isolated resource root");
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).expect("resource directory");
    let reserved = TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let addr = reserved.local_addr().expect("reserved address");
    drop(reserved);
    let bin = release_binary();
    let child = Command::new(bin)
      .args(["epic", "serve", "--http", &addr.to_string()])
      .env("TOOLU_RESOURCE_HOME", &resources)
      .env("HOME", root.path())
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()
      .expect("launch release engine");
    let mut server = Self {
      child,
      addr,
      _root: root,
      streams: Vec::new(),
    };
    server.wait_ready();
    server
  }

  fn wait_ready(&mut self) {
    for _ in 0..200 {
      if TcpStream::connect(self.addr).is_ok() {
        return;
      }
      if let Some(exit) = self.child.try_wait().expect("poll engine") {
        panic!("engine exited before listener: {exit}");
      }
      thread::sleep(Duration::from_millis(10));
    }
    panic!("status listener did not start");
  }

  fn open_streams(&mut self) {
    for _ in 0..STREAMS {
      let mut stream = TcpStream::connect(self.addr).expect("SSE connect");
      stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("SSE timeout");
      let request = format!(
        "GET /api/v1/events/stream HTTP/1.1\r\nHost: {}\r\n\r\n",
        self.addr
      );
      stream.write_all(request.as_bytes()).expect("SSE request");
      let mut line = String::new();
      BufReader::new(stream.try_clone().expect("SSE clone"))
        .read_line(&mut line)
        .expect("SSE response");
      assert!(line.starts_with("HTTP/1.1 200"), "{line}");
      self.streams.push(stream);
    }
  }

  fn high_water_kib(&self) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{}/status", self.child.id()))
      .expect("Linux process status");
    status
      .lines()
      .find_map(|line| line.strip_prefix("VmHWM:"))
      .and_then(|line| line.split_whitespace().next())
      .and_then(|value| value.parse::<u64>().ok())
      .expect("VmHWM in KiB")
  }
}

fn release_binary() -> PathBuf {
  if let Some(path) = std::env::var_os("TOOLU_RELEASE_BIN") {
    return PathBuf::from(path);
  }
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let target = std::env::var_os("CARGO_TARGET_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| root.join("target"))
    .join("status-budget");
  let output = Command::new("cargo")
    .args(["build", "--release", "--locked", "-p", "toolu-cli"])
    .arg("--manifest-path")
    .arg(root.join("Cargo.toml"))
    .arg("--target-dir")
    .arg(&target)
    .env_remove("RUSTFLAGS")
    .env_remove("CARGO_ENCODED_RUSTFLAGS")
    .output()
    .expect("build release status binary");
  assert!(
    output.status.success(),
    "release build failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  target.join("release/toolu")
}

impl Drop for Server {
  fn drop(&mut self) {
    let _killed = self.child.kill();
    let _waited = self.child.wait();
  }
}

fn poll(addr: SocketAddr, barrier: Arc<Barrier>) -> Vec<Duration> {
  let stream = TcpStream::connect(addr).expect("poll connect");
  stream
    .set_read_timeout(Some(Duration::from_secs(10)))
    .expect("poll timeout");
  let mut reader = BufReader::new(stream);
  let request =
    format!("GET /api/v1/overview HTTP/1.1\r\nHost: {addr}\r\nConnection: keep-alive\r\n\r\n");
  barrier.wait();
  let mut samples = Vec::with_capacity(REQUESTS);
  for _ in 0..REQUESTS {
    let start = Instant::now();
    reader
      .get_mut()
      .write_all(request.as_bytes())
      .expect("poll request");
    read_response(&mut reader);
    samples.push(start.elapsed());
  }
  samples
}

fn read_response(reader: &mut BufReader<TcpStream>) {
  let mut line = String::new();
  reader.read_line(&mut line).expect("status line");
  assert!(line.starts_with("HTTP/1.1 200"), "{line}");
  let mut length = None;
  loop {
    line.clear();
    reader.read_line(&mut line).expect("header line");
    if line == "\r\n" {
      break;
    }
    if let Some(value) = line.strip_prefix("Content-Length: ") {
      length = Some(value.trim().parse::<usize>().expect("body length"));
    }
  }
  let mut body = vec![0; length.expect("content length")];
  reader.read_exact(&mut body).expect("status body");
  assert!(body.starts_with(b"{"));
}

#[test]
fn status_server_meets_linux_polling_budget() {
  let mut server = Server::start();
  server.open_streams();
  let barrier = Arc::new(Barrier::new(CLIENTS));
  let handles: Vec<_> = (0..CLIENTS)
    .map(|_| {
      let barrier = Arc::clone(&barrier);
      let addr = server.addr;
      thread::spawn(move || poll(addr, barrier))
    })
    .collect();
  let mut samples = Vec::with_capacity(CLIENTS * REQUESTS);
  for handle in handles {
    samples.extend(handle.join().expect("poll client"));
  }
  samples.sort_unstable();
  let p90 = samples[(samples.len() * 9).div_ceil(10) - 1];
  let high_water_kib = server.high_water_kib();
  println!(
    "status budget: p90={} ms; VmHWM={} KiB; samples={}",
    p90.as_secs_f64() * 1_000.0,
    high_water_kib,
    samples.len()
  );
  assert!(p90 <= Duration::from_millis(7), "poll p90 exceeded 7 ms");
  assert!(high_water_kib <= 13 * 1024, "VmHWM exceeded 13 MiB");
}
