//! The parent side of `interleave.rs`: a sandbox per race, writer processes of
//! both implementations, a reader polling the file meanwhile, and the checks.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Output, Stdio};
use std::time::{Duration, Instant, SystemTime};

use toolu_runtime::json::ordered::Ordered;
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::GateFile;
use toolu_state::lock::lock_path;

use crate::cases::{field, text};
use crate::sandbox::{Res, Sandbox};
use crate::writers::{Writer, number};

/// No single operation of a Rust writer may wait this long (a stale-lock stall).
const STALL: Duration = Duration::from_secs(2);

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
    let mut children = Vec::new();
    for writer in writers {
      children.push((writer, self.spawn(writer, count, &mode)?));
    }
    let (reads, bad) = self.watch(&mut children)?;
    if reads <= number(case, "minReads")? || !bad.is_empty() {
      return Err(format!("{reads} reads, bad: {bad:?}"));
    }
    for (writer, child) in children {
      check_writer(
        writer,
        &child.wait_with_output().map_err(|err| err.to_string())?,
      )?;
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

  /// Polls the file until every child exits: reads made, and reads that failed.
  fn watch(&self, children: &mut [(&Writer, Child)]) -> Res<(usize, Vec<String>)> {
    let (mut reads, mut bad) = (0, Vec::new());
    loop {
      match read_gate_file(&self.gate) {
        GateRead::Missing | GateRead::Ok(_) => {}
        GateRead::Malformed(reason) => bad.push(format!("malformed: {reason}")),
        GateRead::Unrecognized { reason, .. } => bad.push(format!("unrecognized: {reason}")),
      }
      reads += 1;
      let mut running = false;
      for (_, child) in children.iter_mut() {
        running |= child.try_wait().map_err(|err| err.to_string())?.is_none();
      }
      if !running {
        return Ok((reads, bad));
      }
      std::thread::sleep(Duration::from_millis(1));
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
    let started = Instant::now();
    let writer = Writer::rust(&text(case, "writerId")?);
    let child = self.spawn(&writer, number(case, "iterations")?, &text(case, "mode")?)?;
    check_writer(
      &writer,
      &child.wait_with_output().map_err(|err| err.to_string())?,
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
    let started = Instant::now();
    let child = self.spawn(after, 2, "strict")?;
    check_writer(
      after,
      &child.wait_with_output().map_err(|err| err.to_string())?,
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

/// The lock exists and names a pid and a version 4 UUID, as both writers write it.
fn lock_held(lock: &Path) -> bool {
  let Ok(content) = std::fs::read_to_string(lock) else {
    return false;
  };
  let Some((pid, token)) = content
    .strip_suffix('\n')
    .and_then(|line| line.split_once(' '))
  else {
    return false;
  };
  let uuid = token.len() == 36
    && token
      .bytes()
      .all(|byte| byte.is_ascii_hexdigit() || byte == b'-');
  !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit()) && uuid
}

fn numbers(value: &Ordered) -> Vec<String> {
  let Ordered::Array(items) = value else {
    return Vec::new();
  };
  items
    .iter()
    .filter_map(|item| {
      if let Ordered::Number(n) = item {
        Some(n.to_string())
      } else {
        None
      }
    })
    .collect()
}

/// A writer exited 0 with nothing on stderr; a Rust writer never stalled.
fn check_writer(writer: &Writer, out: &Output) -> Res<()> {
  let stderr = String::from_utf8_lossy(&out.stderr);
  if !out.status.success() || !stderr.is_empty() {
    return Err(format!(
      "writer {} failed ({}): {stderr}",
      writer.id(),
      out.status
    ));
  }
  let stdout = String::from_utf8_lossy(&out.stdout);
  let slowest = stdout
    .trim()
    .strip_prefix("slowest_ms=")
    .and_then(|ms| ms.parse::<u64>().ok());
  match (writer, slowest) {
    (Writer::Rust(id), Some(ms)) if Duration::from_millis(ms) >= STALL => {
      Err(format!("writer {id} stalled {ms} ms"))
    }
    (Writer::Rust(id), None) => Err(format!("writer {id} printed {stdout:?}")),
    _ => Ok(()),
  }
}
