//! Real Linux status-listener budget under 50 polling clients and eight SSE readers.

#![cfg(target_os = "linux")]

use std::io::{self, BufRead as _, BufReader, Read as _, Write as _};
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
  fn start() -> io::Result<Self> {
    let root = contextual(tempfile::tempdir(), "create isolated resource root")?;
    let resources = root.path().join("resources");
    contextual(
      std::fs::create_dir_all(&resources),
      "create resource directory",
    )?;
    let reserved = contextual(TcpListener::bind("127.0.0.1:0"), "reserve port")?;
    let addr = contextual(reserved.local_addr(), "read reserved address")?;
    drop(reserved);
    let bin = release_binary()?;
    let child = Command::new(bin)
      .args(["epic", "serve", "--http", &addr.to_string()])
      .env("TOOLU_RESOURCE_HOME", &resources)
      .env("HOME", root.path())
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn();
    let child = contextual(child, "launch release engine")?;
    let mut server = Self {
      child,
      addr,
      _root: root,
      streams: Vec::new(),
    };
    server.wait_ready()?;
    Ok(server)
  }

  fn wait_ready(&mut self) -> io::Result<()> {
    for _ in 0..200 {
      if TcpStream::connect(self.addr).is_ok() {
        return Ok(());
      }
      if let Some(exit) = contextual(self.child.try_wait(), "poll release engine")? {
        return Err(io::Error::other(format!(
          "engine exited before listener: {exit}"
        )));
      }
      thread::sleep(Duration::from_millis(10));
    }
    Err(io::Error::other("status listener did not start"))
  }

  fn open_streams(&mut self) -> io::Result<()> {
    for _ in 0..STREAMS {
      let mut stream = contextual(TcpStream::connect(self.addr), "connect SSE reader")?;
      contextual(
        stream.set_read_timeout(Some(Duration::from_secs(5))),
        "set SSE read timeout",
      )?;
      let request = format!(
        "GET /api/v1/events/stream HTTP/1.1\r\nHost: {}\r\n\r\n",
        self.addr
      );
      contextual(stream.write_all(request.as_bytes()), "send SSE request")?;
      let mut line = String::new();
      let clone = contextual(stream.try_clone(), "clone SSE socket")?;
      contextual(
        BufReader::new(clone).read_line(&mut line),
        "read SSE response",
      )?;
      assert_http_ok(&line);
      self.streams.push(stream);
    }
    Ok(())
  }

  fn high_water_kib(&self) -> io::Result<u64> {
    let status = contextual(
      std::fs::read_to_string(format!("/proc/{}/status", self.child.id())),
      "read Linux process status",
    )?;
    status
      .lines()
      .find_map(|line| line.strip_prefix("VmHWM:"))
      .and_then(|line| line.split_whitespace().next())
      .ok_or_else(|| io::Error::other("VmHWM missing from Linux process status"))
      .and_then(|value| contextual(value.parse::<u64>(), "parse VmHWM in KiB"))
  }
}

fn contextual<T, E: std::fmt::Display>(result: Result<T, E>, action: &str) -> io::Result<T> {
  result.map_err(|error| io::Error::other(format!("{action}: {error}")))
}

fn release_binary() -> io::Result<PathBuf> {
  if let Some(path) = std::env::var_os("TOOLU_RELEASE_BIN") {
    return Ok(PathBuf::from(path));
  }
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let target = std::env::var_os("CARGO_TARGET_DIR")
    .map_or_else(|| root.join("target"), PathBuf::from)
    .join("status-budget");
  let output = Command::new("cargo")
    .args(["build", "--release", "--locked", "-p", "toolu-cli"])
    .arg("--manifest-path")
    .arg(root.join("Cargo.toml"))
    .arg("--target-dir")
    .arg(&target)
    .env_remove("RUSTFLAGS")
    .env_remove("CARGO_ENCODED_RUSTFLAGS")
    .output();
  let output = contextual(output, "build release status binary")?;
  if !output.status.success() {
    return Err(io::Error::other(format!(
      "release build failed: {}",
      String::from_utf8_lossy(&output.stderr)
    )));
  }
  Ok(target.join("release/toolu"))
}

impl Drop for Server {
  fn drop(&mut self) {
    let _killed = self.child.kill();
    let _waited = self.child.wait();
  }
}

fn poll(addr: SocketAddr, barrier: &Barrier) -> io::Result<Vec<Duration>> {
  let stream = TcpStream::connect(addr);
  barrier.wait();
  let stream = contextual(stream, "connect polling client")?;
  contextual(
    stream.set_read_timeout(Some(Duration::from_secs(10))),
    "set polling read timeout",
  )?;
  let mut reader = BufReader::new(stream);
  let request =
    format!("GET /api/v1/overview HTTP/1.1\r\nHost: {addr}\r\nConnection: keep-alive\r\n\r\n");
  let mut samples = Vec::with_capacity(REQUESTS);
  for _ in 0..REQUESTS {
    let start = Instant::now();
    contextual(
      reader.get_mut().write_all(request.as_bytes()),
      "send poll request",
    )?;
    read_response(&mut reader)?;
    samples.push(start.elapsed());
  }
  Ok(samples)
}

fn read_response(reader: &mut BufReader<TcpStream>) -> io::Result<()> {
  let mut line = String::new();
  contextual(reader.read_line(&mut line), "read HTTP status line")?;
  assert_http_ok(&line);
  let mut length = None;
  loop {
    line.clear();
    let read = contextual(reader.read_line(&mut line), "read HTTP header line")?;
    if read == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "HTTP headers ended before the blank line",
      ));
    }
    if line == "\r\n" {
      break;
    }
    if let Some(value) = line.strip_prefix("Content-Length: ") {
      length = Some(contextual(
        value.trim().parse::<usize>(),
        "parse HTTP body length",
      )?);
    }
  }
  let length = length.ok_or_else(|| io::Error::other("HTTP content length missing"))?;
  let mut body = vec![0; length];
  contextual(reader.read_exact(&mut body), "read HTTP status body")?;
  assert_json_body(&body);
  Ok(())
}

fn assert_http_ok(line: &str) {
  assert!(line.starts_with("HTTP/1.1 200"), "{line}");
}

fn assert_json_body(body: &[u8]) {
  assert!(body.starts_with(b"{"));
}

fn assert_budget(p90: Duration, high_water_kib: u64) {
  assert!(p90 <= Duration::from_millis(7), "poll p90 exceeded 7 ms");
  assert!(high_water_kib <= 13 * 1024, "VmHWM exceeded 13 MiB");
}

#[test]
fn status_server_meets_linux_polling_budget() -> io::Result<()> {
  let mut server = Server::start()?;
  server.open_streams()?;
  let barrier = Arc::new(Barrier::new(CLIENTS));
  let handles: Vec<_> = (0..CLIENTS)
    .map(|_| {
      let barrier = Arc::clone(&barrier);
      let addr = server.addr;
      thread::spawn(move || poll(addr, &barrier))
    })
    .collect();
  let mut samples = Vec::with_capacity(CLIENTS * REQUESTS);
  for handle in handles {
    let client = handle.join().map_err(|panic| {
      let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload");
      io::Error::other(format!("poll client thread panicked: {message}"))
    })?;
    samples.extend(client?);
  }
  samples.sort_unstable();
  let p90 = samples[(samples.len() * 9).div_ceil(10) - 1];
  let high_water_kib = server.high_water_kib()?;
  println!(
    "status budget: p90={} ms; VmHWM={} KiB; samples={}",
    p90.as_secs_f64() * 1_000.0,
    high_water_kib,
    samples.len()
  );
  assert_budget(p90, high_water_kib);
  Ok(())
}
