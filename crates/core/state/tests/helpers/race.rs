//! The parent side of `interleave.rs`: a sandbox per race, writer processes of
//! both implementations, a reader polling the file meanwhile, and the checks.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant, SystemTime};

use toolu_runtime::json::ordered::Ordered;
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::GateFile;
use toolu_state::lock::lock_path;

use crate::cases::{field, text};
use crate::fleet::{Fleet, STALL, check_writer, finished, lock_held, numbers};
use crate::sandbox::{Res, Sandbox};
use crate::writers::{Writer, number};

/// One sandbox and its gate file.
pub(crate) struct Race {
  sb: Sandbox,
  gate: PathBuf,
}

impl Race {
  /// A repository on `branch` with a project config, as the TypeScript race sets it up.
  pub(crate) fn new(branch: &str) -> Res<Race> {
    let sb = Sandbox::new(Some(branch))?;
    let dir = sb.project.join(".claude/tmp");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let config = sb.project.join(".claude/toolu.config.json");
    std::fs::write(config, "{\"version\":1}").map_err(|err| err.to_string())?;
    Ok(Race {
      gate: dir.join("quality-gate-status.json"),
      sb,
    })
  }

  fn spawn(&self, writer: &Writer, count: usize, mode: &str) -> Res<Child> {
    let mut command = writer.command(&self.gate, count, mode)?;
    command
      .current_dir(&self.sb.project)
      .env_clear()
      .envs(self.sb.env().vars())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    command.spawn().map_err(|err| {
      format!(
        "cannot start writer {}: {err} (bun is required)",
        writer.id()
      )
    })
  }

  /// Runs `writers` at once while a reader polls; checks every outcome.
  pub(crate) fn run(&self, writers: &[Writer], case: &Ordered) -> Res<()> {
    let (count, mode) = (number(case, "iterations")?, text(case, "mode")?);
    let mut fleet = Fleet(Vec::new());
    let ordered = writers
      .iter()
      .filter(|writer| !writer.is_typescript())
      .chain(writers.iter().filter(|writer| writer.is_typescript()));
    let mut started_typescript = false;
    for writer in ordered {
      if writer.is_typescript() && !started_typescript {
        self.wait_rust_ready(&mut fleet)?;
        started_typescript = true;
      }
      fleet.0.push((writer, self.spawn(writer, count, &mode)?));
    }
    let (reads, bad) = self.watch(&mut fleet)?;
    if reads <= number(case, "minReads")? || !bad.is_empty() {
      return Err(format!("{reads} reads, bad: {bad:?}"));
    }
    // With TypeScript writers in the race, every Rust writer must have kept
    // writing until the last of them exited: that is the interleaving.
    let overlap = writers.iter().any(Writer::is_typescript);
    for (writer, child) in &mut fleet.0 {
      check_writer(writer, &finished(child)?, overlap)?;
    }
    let kept = numbers(field(case, "keptIndices")?);
    let expected: Vec<String> = writers
      .iter()
      .flat_map(|writer| {
        kept
          .iter()
          .map(move |index| format!("/w/{}/{index}", writer.id()))
      })
      .collect();
    self.check_final(expected)
  }

  /// Each Rust writer has reached its cycle before the short TypeScript writers start.
  fn wait_rust_ready(&self, fleet: &mut Fleet<'_>) -> Res<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    for (writer, child) in &mut fleet.0 {
      self.wait_writer_ready(writer, child, deadline)?;
    }
    Ok(())
  }

  fn wait_writer_ready(&self, writer: &Writer, child: &mut Child, deadline: Instant) -> Res<()> {
    let ready = PathBuf::from(format!(
      "{}.stop.ready-{}",
      self.gate.display(),
      writer.id()
    ));
    while !ready.exists() {
      if child.try_wait().map_err(|err| err.to_string())?.is_some() {
        return Err(format!(
          "writer {} exited before the mixed race",
          writer.id()
        ));
      }
      if Instant::now() >= deadline {
        return Err(format!("writer {} never became ready", writer.id()));
      }
      std::thread::sleep(Duration::from_millis(1));
    }
    std::fs::remove_file(ready).map_err(|err| err.to_string())
  }

  /// Polls the file until every child exits; tells the Rust writers to stop
  /// cycling once every TypeScript writer is done. Reads made, and reads failed.
  fn watch(&self, fleet: &mut Fleet<'_>) -> Res<(usize, Vec<String>)> {
    let stop = PathBuf::from(format!("{}.stop", self.gate.display()));
    let (mut reads, mut bad) = (0, Vec::new());
    loop {
      match read_gate_file(&self.gate) {
        GateRead::Missing | GateRead::Ok(_) => {}
        GateRead::Malformed(reason) => bad.push(format!("malformed: {reason}")),
        GateRead::Unrecognized { reason, .. } => bad.push(format!("unrecognized: {reason}")),
      }
      reads += 1;
      let (mut running, mut typescript) = (false, false);
      for (writer, child) in &mut fleet.0 {
        let live = child.try_wait().map_err(|err| err.to_string())?.is_none();
        running |= live;
        typescript |= live && writer.is_typescript();
      }
      if !typescript && !stop.exists() {
        std::fs::write(&stop, "").map_err(|err| err.to_string())?;
      }
      if !running {
        std::fs::remove_file(&stop).map_err(|err| err.to_string())?;
        return Ok((reads, bad));
      }
      std::thread::sleep(Duration::from_millis(1));
    }
  }

  /// Tells Rust writers not to cycle: no TypeScript writer runs beside them.
  fn stop_cycling(&self) -> Res<()> {
    std::fs::write(format!("{}.stop", self.gate.display()), "").map_err(|err| err.to_string())
  }

  /// Every slot of `slots` is in the file.
  pub(crate) fn has_slots(&self, slots: &[&str]) -> Res<()> {
    let GateRead::Ok(GateFile::Failing {
      entries: Some(entries),
      ..
    }) = read_gate_file(&self.gate)
    else {
      return Err("no slots".into());
    };
    let missing: Vec<&&str> = slots
      .iter()
      .filter(|slot| !entries.iter().any(|(key, _)| key == **slot))
      .collect();
    if missing.is_empty() {
      Ok(())
    } else {
      Err(format!("missing {missing:?}"))
    }
  }

  /// Every expected slot is there, the violations are theirs, nothing is left over.
  fn check_final(&self, mut expected: Vec<String>) -> Res<()> {
    let GateRead::Ok(GateFile::Failing {
      entries: Some(entries),
      violations,
      ..
    }) = read_gate_file(&self.gate)
    else {
      return Err("the final file is not a failing multi-slot document".into());
    };
    let mut keys: Vec<String> = entries.into_iter().map(|(key, _)| key).collect();
    let mut lines: Vec<String> = violations.lines().map(str::to_owned).collect();
    let mut wanted: Vec<String> = expected
      .iter()
      .map(|file| file.trim_start_matches("/w/").to_owned())
      .collect();
    for list in [&mut keys, &mut lines, &mut expected, &mut wanted] {
      list.sort();
    }
    if keys != expected || lines != wanted {
      return Err(format!(
        "slots {keys:?} (want {expected:?}), violations {lines:?}"
      ));
    }
    let dir = self.gate.parent().ok_or("no dir")?;
    let leftovers: Vec<String> = std::fs::read_dir(dir)
      .map_err(|err| err.to_string())?
      .flatten()
      .map(|entry| entry.file_name().to_string_lossy().into_owned())
      .filter(|name| {
        Path::new(name)
          .extension()
          .is_some_and(|ext| ext == "tmp" || ext == "lock")
      })
      .collect();
    if leftovers.is_empty() {
      Ok(())
    } else {
      Err(format!("left over: {leftovers:?}"))
    }
  }
}

/// The lock scenarios: a stale lock left behind, and a holder killed mid-hold.
impl Race {
  /// The fixture's crashed-writer lock: one Rust writer gets past it quickly.
  pub(crate) fn stale_lock(&self, case: &Ordered) -> Res<()> {
    let lock = lock_path(&self.gate);
    std::fs::write(&lock, text(case, "lock")?).map_err(|err| err.to_string())?;
    let age =
      Duration::from_millis(u64::try_from(number(case, "ageMs")?).map_err(|err| err.to_string())?);
    let file = File::options()
      .write(true)
      .open(&lock)
      .map_err(|err| err.to_string())?;
    file
      .set_modified(SystemTime::now() - age)
      .map_err(|err| err.to_string())?;
    self.stop_cycling()?;
    let started = Instant::now();
    let writer = Writer::rust(&text(case, "writerId")?);
    let child = self.spawn(&writer, number(case, "iterations")?, &text(case, "mode")?)?;
    check_writer(
      &writer,
      &child.wait_with_output().map_err(|err| err.to_string())?,
      false,
    )?;
    let max = u64::try_from(number(case, "maxMs")?).map_err(|err| err.to_string())?;
    if started.elapsed() >= Duration::from_millis(max) || lock.exists() {
      return Err("a stale lock held the writer up, or was left behind".into());
    }
    let expected: Vec<String> = match field(case, "expectedEntries")? {
      Ordered::Array(items) => items
        .iter()
        .filter_map(|item| {
          if let Ordered::String(s) = item {
            Some(s.clone())
          } else {
            None
          }
        })
        .collect(),
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Object(_) => Vec::new(),
    };
    self.check_final(expected)
  }

  /// `holder` takes the lock and is killed (and reaped); `after` must then
  /// get through within the stall bound, and its slot must land.
  pub(crate) fn crash_then(&self, holder: &Writer, after: &Writer) -> Res<()> {
    let mut held = self.spawn(holder, 0, "strict")?;
    let lock = lock_path(&self.gate);
    let since = Instant::now();
    while !lock_held(&lock) {
      if since.elapsed() > Duration::from_secs(20) {
        held.kill().ok();
        return Err("the holder never took the lock".into());
      }
      std::thread::sleep(Duration::from_millis(5));
    }
    held.kill().map_err(|err| err.to_string())?;
    held.wait().map_err(|err| err.to_string())?;
    self.stop_cycling()?;
    let started = Instant::now();
    let child = self.spawn(after, 2, "strict")?;
    check_writer(
      after,
      &child.wait_with_output().map_err(|err| err.to_string())?,
      false,
    )?;
    if started.elapsed() >= STALL || lock.exists() {
      return Err(format!(
        "a dead holder's lock held {} up for {:?}",
        after.id(),
        started.elapsed()
      ));
    }
    let GateRead::Ok(GateFile::Failing {
      entries: Some(entries),
      ..
    }) = read_gate_file(&self.gate)
    else {
      return Err("no slots".into());
    };
    let slot = format!("/w/{}/1", after.id());
    if entries.iter().any(|(key, _)| *key == slot) {
      Ok(())
    } else {
      Err(format!("{slot} missing"))
    }
  }
}
