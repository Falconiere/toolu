//! TypeScript and Rust writers on one gate file (AC-1, AC-2, and the
//! `concurrency` cases of `fixtures/state/cases.json`). This test binary has
//! no libtest harness: run plainly it is the parent; `interleave writer …`
//! is one Rust writer process (TypeScript's `gate-writer.ts`, in Rust), and
//! `interleave hold <gate>` takes the lock and holds it for a minute, to be killed.
//! Readers poll the file throughout; it must parse under the strict schema
//! after every write, keep every slot, and leave no temp file or lock.
//!
//! `main` is not test code to clippy, so it unwraps nothing and returns errors.

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/fleet.rs"]
mod fleet;
#[path = "helpers/race.rs"]
mod race;
#[path = "helpers/sandbox.rs"]
mod sandbox;
#[path = "helpers/writers.rs"]
mod writers;

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use cases::{cases_of, text};
use race::Race;
use sandbox::Res;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{ClearOutcome, GateFailure, clear_gate_file, record_gate_failure};
use toolu_state::lock::{LockOptions, with_lock};
use writers::Writer;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let rest = args.get(1..).unwrap_or(&[]);
  let result = match args.first().map(String::as_str) {
    Some("writer") => writer(rest),
    Some("hold") => hold(rest),
    _ => parent(),
  };
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      std::io::stderr()
        .write_all(format!("interleave: {err}\n").as_bytes())
        .ok();
      ExitCode::FAILURE
    }
  }
}

/// One Rust writer's state: its context, file and slowest operation so far.
struct RustWriter<'a> {
  ctx: StateCtx,
  gate: &'a Path,
  source: String,
  reason: String,
  slowest: Duration,
}

impl RustWriter<'_> {
  fn record(&mut self, file: &str, violations: &str) {
    let started = Instant::now();
    let failure = GateFailure {
      file,
      source: &self.source,
      reason: &self.reason,
      violations,
    };
    record_gate_failure(&mut self.ctx, self.gate, &failure);
    self.slowest = self.slowest.max(started.elapsed());
  }

  fn clear(&mut self, file: &str) -> ClearOutcome {
    let started = Instant::now();
    let outcome = clear_gate_file(&mut self.ctx, self.gate, file, &self.source);
    self.slowest = self.slowest.max(started.elapsed());
    outcome
  }
}

/// One Rust writer: records `/w/<id>/0..count`, clears the even ones, then
/// records and clears `/w/<id>/cycle` until `<stop>` exists, so it is still
/// writing for as long as any TypeScript writer runs. Prints its slowest
/// operation and how many cycles it ran.
fn writer(args: &[String]) -> Res<()> {
  let [gate, id, count, mode, stop] = args else {
    return Err("usage: writer <gate> <id> <count> <mode> <stop>".into());
  };
  let count: usize = count
    .parse()
    .map_err(|err| format!("bad count {count}: {err}"))?;
  let mut w = RustWriter {
    ctx: StateCtx::new(Roots::new(Env::process(), Some(Host::Claude))),
    gate: Path::new(gate),
    source: format!("writer-{id}"),
    reason: format!("reason {id}"),
    slowest: Duration::ZERO,
  };
  for n in 0..count {
    w.record(&format!("/w/{id}/{n}"), &format!("{id}/{n}\n"));
  }
  for n in (0..count).step_by(2) {
    if w.clear(&format!("/w/{id}/{n}")) == ClearOutcome::Noop && mode == "strict" {
      return Err(format!("writer {id}: clear of {n} was a no-op"));
    }
  }
  let (cycle, deadline) = (
    format!("/w/{id}/cycle"),
    Instant::now() + Duration::from_secs(60),
  );
  let mut cycles = 0_u32;
  while !Path::new(stop).exists() && Instant::now() < deadline {
    w.record(&cycle, "cycle\n");
    if w.clear(&cycle) == ClearOutcome::Noop {
      return Err(format!("writer {id}: a cycle's clear was a no-op"));
    }
    cycles += 1;
    std::thread::sleep(Duration::from_millis(2));
  }
  if !w.ctx.warnings.is_empty() {
    return Err(format!("writer {id} warned: {:?}", w.ctx.warnings));
  }
  let line = format!("slowest_ms={} cycles={cycles}\n", w.slowest.as_millis());
  std::io::stdout()
    .write_all(line.as_bytes())
    .map_err(|err| err.to_string())
}

/// Takes the gate's lock and holds it until killed (or a minute passes).
fn hold(args: &[String]) -> Res<()> {
  let [gate] = args else {
    return Err("usage: hold <gate>".into());
  };
  let until = Instant::now() + Duration::from_secs(60);
  with_lock(
    Path::new(gate),
    LockOptions::default(),
    &mut Vec::new(),
    || {
      while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
      }
    },
  );
  Ok(())
}

fn parent() -> Res<()> {
  fixture_cases()?;
  mixed_race()?;
  crashed_holders()?;
  std::io::stdout()
    .write_all(b"interleave: ok\n")
    .map_err(|err| err.to_string())
}

/// The two `concurrency` cases, with Rust writers.
fn fixture_cases() -> Res<()> {
  for case in cases_of("state/cases.json", "concurrency")? {
    let race = Race::new(&text(&case, "branch")?)?;
    if text(&case, "scenario")? == "race" {
      let count = writers::number(&case, "writers")?;
      let all: Vec<Writer> = (0..count).map(|id| Writer::rust(&id.to_string())).collect();
      race.run(&all, &case)?;
    } else {
      race.stale_lock(&case)?;
    }
  }
  Ok(())
}

/// AC-1: eight TypeScript and eight Rust writers on one file.
fn mixed_race() -> Res<()> {
  let case = cases_of("state/cases.json", "concurrency")?
    .into_iter()
    .find(|case| text(case, "scenario").as_deref() == Ok("race"))
    .ok_or("no race case")?;
  let race = Race::new("feat/race")?;
  let all: Vec<Writer> = (0..16)
    .map(|id| {
      if id < 8 {
        Writer::typescript(&id.to_string())
      } else {
        Writer::rust(&id.to_string())
      }
    })
    .collect();
  race.run(&all, &case)
}

/// AC-2: a lock left by a killed holder of one implementation is broken at
/// once by a writer of the other.
fn crashed_holders() -> Res<()> {
  let race = Race::new("feat/crash")?;
  race.crash_then(&Writer::rust_holder(), &Writer::typescript("ts-after-rust"))?;
  race.crash_then(&Writer::typescript_holder(), &Writer::rust("rust-after-ts"))?;
  race.has_slots(&["/w/ts-after-rust/1", "/w/rust-after-ts/1"])
}
