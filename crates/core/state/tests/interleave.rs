//! TypeScript and Rust writers on one gate file (AC-1, AC-2, and the
//! `concurrency` cases of `fixtures/state/cases.json`). This test binary has
//! no libtest harness: run plainly it is the parent; `interleave writer …`
//! is one Rust writer process (TypeScript's `gate-writer.ts`, in Rust), and
//! `interleave hold <gate>` takes the lock and never lets go, to be killed.
//! Readers poll the file throughout; it must parse under the strict schema
//! after every write, keep every slot, and leave no temp file or lock.
//!
//! `main` is not test code to clippy, so it unwraps nothing and returns errors.

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/sandbox.rs"]
mod sandbox;
#[path = "helpers/writers.rs"]
mod writers;

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use cases::{cases_of, text};
use sandbox::Res;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{ClearOutcome, GateFailure, clear_gate_file, record_gate_failure};
use toolu_state::lock::{LockOptions, with_lock};
use writers::{Race, Writer};

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

/// One Rust writer: records `/w/<id>/0..count`, then clears the even ones,
/// and prints its slowest operation in milliseconds.
fn writer(args: &[String]) -> Res<()> {
  let [gate, id, count, mode] = args else {
    return Err("usage: writer <gate> <id> <count> <mode>".into());
  };
  let count: usize = count
    .parse()
    .map_err(|err| format!("bad count {count}: {err}"))?;
  let mut ctx = StateCtx::new(Roots::new(Env::process(), Some(Host::Claude)));
  let gate = Path::new(gate);
  let (source, reason) = (format!("writer-{id}"), format!("reason {id}"));
  let mut slowest = Duration::ZERO;
  for n in 0..count {
    let (file, violations) = (format!("/w/{id}/{n}"), format!("{id}/{n}\n"));
    let started = Instant::now();
    let failure = GateFailure {
      file: &file,
      source: &source,
      reason: &reason,
      violations: &violations,
    };
    record_gate_failure(&mut ctx, gate, &failure);
    slowest = slowest.max(started.elapsed());
  }
  for n in (0..count).step_by(2) {
    let started = Instant::now();
    let outcome = clear_gate_file(&mut ctx, gate, &format!("/w/{id}/{n}"), &source);
    slowest = slowest.max(started.elapsed());
    if outcome == ClearOutcome::Noop && mode == "strict" {
      return Err(format!("writer {id}: clear of {n} was a no-op"));
    }
  }
  if !ctx.warnings.is_empty() {
    return Err(format!("writer {id} warned: {:?}", ctx.warnings));
  }
  std::io::stdout()
    .write_all(format!("slowest_ms={}\n", slowest.as_millis()).as_bytes())
    .map_err(|err| err.to_string())
}

/// Takes the gate's lock and holds it until killed.
fn hold(args: &[String]) -> Res<()> {
  let [gate] = args else {
    return Err("usage: hold <gate>".into());
  };
  with_lock(
    Path::new(gate),
    LockOptions::default(),
    &mut Vec::new(),
    || loop {
      std::thread::sleep(Duration::from_secs(1));
    },
  )
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
  race.crash_then(&Writer::typescript_holder(), &Writer::rust("rust-after-ts"))
}
