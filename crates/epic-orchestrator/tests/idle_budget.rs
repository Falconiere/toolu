//! The idle engine budget of #451 on a real Linux release binary: with no epic
//! and no client, `toolu epic serve --http` stays at 10 MB resident or less and
//! has no timer under 30 s. Each idle source (the 30 s tick, the herdr thread's
//! 30 s pause with no herdr socket) fits at most twice in a 35 s window, so the
//! whole process makes at most 20 voluntary context switches in it; the old
//! 20 ms accept poll made about 1 750 and a single 1 s timer makes about 35.

#![cfg(target_os = "linux")]

#[path = "common/release.rs"]
mod release;

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use release::{contextual, release_binary};

const WARM_UP: Duration = Duration::from_secs(2);
const WINDOW: Duration = Duration::from_secs(35);
const MAX_SWITCHES: u64 = 20;
const MAX_RESIDENT_BYTES: u64 = 10_000_000;

struct Idle {
  child: Child,
  _root: tempfile::TempDir,
}

impl Drop for Idle {
  fn drop(&mut self) {
    let _killed = self.child.kill();
    let _waited = self.child.wait();
  }
}

fn start() -> io::Result<Idle> {
  let root = contextual(tempfile::tempdir(), "create isolated resource root")?;
  let resources = root.path().join("resources");
  contextual(std::fs::create_dir_all(&resources), "create resources")?;
  let reserved = contextual(TcpListener::bind("127.0.0.1:0"), "reserve port")?;
  let addr = contextual(reserved.local_addr(), "read reserved address")?;
  drop(reserved);
  let child = Command::new(release_binary()?)
    .args(["epic", "serve", "--http", &addr.to_string()])
    .env("TOOLU_RESOURCE_HOME", &resources)
    .env("HOME", root.path())
    .env_remove("TOOLU_EPIC_HERDR")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn();
  let idle = Idle {
    child: contextual(child, "launch release engine")?,
    _root: root,
  };
  wait_listening(addr)?;
  Ok(idle)
}

fn wait_listening(addr: SocketAddr) -> io::Result<()> {
  for _ in 0..500 {
    if TcpStream::connect(addr).is_ok() {
      return Ok(());
    }
    thread::sleep(Duration::from_millis(10));
  }
  Err(io::Error::other("status listener did not start"))
}

/// The sum of `field` over every thread of `pid`.
fn task_sum(pid: u32, field: &str) -> io::Result<u64> {
  let tasks = contextual(std::fs::read_dir(format!("/proc/{pid}/task")), "list tasks")?;
  let mut total = 0_u64;
  for task in tasks {
    let path = contextual(task, "read task entry")?.path().join("status");
    let status = contextual(std::fs::read_to_string(&path), "read task status")?;
    total = total.saturating_add(field_value(&status, field)?);
  }
  Ok(total)
}

fn field_value(status: &str, field: &str) -> io::Result<u64> {
  status
    .lines()
    .find_map(|line| line.strip_prefix(field))
    .and_then(|rest| rest.split_whitespace().next())
    .ok_or_else(|| io::Error::other(format!("{field} missing from status")))
    .and_then(|value| contextual(value.parse::<u64>(), "parse status field"))
}

#[test]
fn an_idle_engine_stays_small_and_wakes_only_on_slow_timers() -> io::Result<()> {
  let idle = start()?;
  let pid = idle.child.id();
  thread::sleep(WARM_UP);
  let before = task_sum(pid, "voluntary_ctxt_switches:")?;
  thread::sleep(WINDOW);
  let switches = task_sum(pid, "voluntary_ctxt_switches:")?.saturating_sub(before);
  let status = contextual(
    std::fs::read_to_string(format!("/proc/{pid}/status")),
    "read process status",
  )?;
  let resident = field_value(&status, "VmHWM:")?.saturating_mul(1024);
  println!("idle budget: {switches} voluntary switches in 35 s; VmHWM={resident} bytes");
  assert!(
    switches <= MAX_SWITCHES,
    "{switches} switches: a timer under 30 s runs"
  );
  assert!(
    resident <= MAX_RESIDENT_BYTES,
    "VmHWM {resident} bytes exceeds 10 MB"
  );
  Ok(())
}
