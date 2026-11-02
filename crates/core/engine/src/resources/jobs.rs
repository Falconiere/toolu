//! Expensive work under a machine-wide job lease (`packages/toolu-core/src/resources/jobs.ts`).
//! The job waits on its stdin for a go line, so its lease records the process
//! group before the work starts; a heartbeat refreshes the lease every 30
//! seconds; a signalled runner takes the group with it. Afterwards the lease is
//! released once no member of the group is left, and otherwise kept as
//! `cleanup-incomplete`, so uncertain ownership stays visible.

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::guard::GroupGuard;
use toolu_runtime::process::{ChunkObserver, Output, Spec, group, run_with_chunks};
use toolu_state::lock::token;
use toolu_state::time::iso_millis;

use super::binding::ResourceBinding;
use super::job_logs;
use super::leases::{
  LeaseKind, LeasePatch, LeaseRequest, acquire_lease, patch_lease, release_lease,
};

/// How often a running job refreshes its lease.
const HEARTBEAT: Duration = Duration::from_secs(30);
/// The shell that waits for the go line, then becomes the job.
const GO_SHELL: &str = "IFS= read -r ack && [ \"$ack\" = toolu-go ] && exec \"$@\"";

/// One failure slot; heartbeats and log writes use separate instances.
type Failure = Arc<Mutex<Option<String>>>;

#[derive(Default)]
struct Failures {
  heartbeat: Failure,
  log: Failure,
}

fn beat(root: &Path, token: &str, failure: &Failure) {
  let patch = LeasePatch {
    heartbeat_at: Some(iso_millis(SystemTime::now())),
    ..LeasePatch::default()
  };
  let outcome = patch_lease(root, token, &patch).err();
  *failure.lock().unwrap_or_else(PoisonError::into_inner) = outcome;
}

/// The heartbeat thread, until its sender is dropped.
fn heartbeat(root: &Path, token: &str, failure: &Failure) -> (Sender<()>, JoinHandle<()>) {
  let (stop, wait) = mpsc::channel::<()>();
  let (root, token, failure) = (root.to_path_buf(), token.to_owned(), Arc::clone(failure));
  let handle = std::thread::spawn(move || {
    while let Err(RecvTimeoutError::Timeout) = wait.recv_timeout(HEARTBEAT) {
      beat(&root, &token, &failure);
    }
  });
  (stop, handle)
}

fn lease_token(lease: &Ordered) -> Result<String, String> {
  match lease.get("token") {
    Some(Ordered::String(token)) => Ok(token.clone()),
    _ => Err("resource lease lost".to_owned()),
  }
}

fn log_observer(log: Arc<job_logs::Writer>, failure: &Failure) -> ChunkObserver {
  let failure = Arc::clone(failure);
  Arc::new(move |bytes| {
    if let Err(err) = log.append(bytes) {
      let mut failed = failure.lock().unwrap_or_else(PoisonError::into_inner);
      if failed.is_none() {
        *failed = Some(err);
      }
    }
  })
}

/// The job's spec: `argv` behind the go-line shell, the go line before its stdin.
fn wrapped(argv: &[String], spec: &Spec) -> Spec {
  let mut job = spec.clone();
  job.argv = ["bash", "-c", GO_SHELL, "toolu-job"]
    .map(str::to_owned)
    .to_vec();
  job.argv.extend(argv.iter().cloned());
  job.stdin = [b"toolu-go\n".as_slice(), &spec.stdin].concat();
  job
}

/// Release the lease when no member of the group is left; else mark it.
fn settle(root: &Path, token: &str, group_id: Option<u32>) -> Result<(), String> {
  match group_id.filter(|id| group::alive(*id)) {
    None => release_lease(root, token),
    Some(_) => patch_lease(
      root,
      token,
      &LeasePatch {
        stage: Some("cleanup-incomplete".to_owned()),
        ..LeasePatch::default()
      },
    ),
  }
}

fn finish(
  root: &Path,
  token: &str,
  group_id: Option<u32>,
  beating: Option<(Sender<()>, JoinHandle<()>)>,
  failures: &Failures,
) -> Result<(), String> {
  if let Some((stop, handle)) = beating {
    drop(stop);
    let _joined = handle.join();
  }
  settle(root, token, group_id)?;
  if let Some(failed) = failures
    .log
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .take()
  {
    return Err(failed);
  }
  if let Some(failed) = failures
    .heartbeat
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .take()
  {
    return Err(failed);
  }
  Ok(())
}

fn leased_log(
  binding: &ResourceBinding,
  token: &str,
  argv: &[String],
) -> Result<Arc<job_logs::Writer>, String> {
  let root = binding.root.as_path();
  match job_logs::start(root, token, &binding.key, argv) {
    Ok(log) => Ok(Arc::new(log)),
    Err(err) => {
      settle(root, token, None)?;
      Err(err)
    }
  }
}

/// What the runner tells its caller about admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission<'a> {
  /// A lease is about to be requested.
  Attempt,
  /// The request was refused for `capacity` or `pressure`; the runner retries
  /// after its poll while its bound allows.
  Refused(&'a str),
}

/// How long a job waits for admission, how often it retries, and who hears it.
pub struct JobWait<'a> {
  /// The longest wait before the last refusal is returned; zero refuses at once.
  pub bound: Duration,
  /// The pause between attempts.
  pub poll: Duration,
  /// Hears every attempt and refusal.
  pub observe: Option<&'a mut dyn FnMut(Admission<'_>)>,
}

/// The default bound of a waiting job.
pub const JOB_WAIT_BOUND: Duration = Duration::from_mins(30);
/// The default pause between attempts: the pressure sample age.
pub const JOB_WAIT_POLL: Duration = Duration::from_secs(30);

impl JobWait<'static> {
  /// Refuse at once, telling nobody.
  #[must_use]
  pub fn none() -> Self {
    JobWait {
      bound: Duration::ZERO,
      poll: Duration::ZERO,
      observe: None,
    }
  }

  /// Wait up to `bound`, retrying every `poll`, telling nobody.
  #[must_use]
  pub fn silent(bound: Duration, poll: Duration) -> Self {
    JobWait {
      bound,
      poll,
      observe: None,
    }
  }
}

impl JobWait<'_> {
  fn tell(&mut self, admission: Admission<'_>) {
    if let Some(observe) = self.observe.as_mut() {
      observe(admission);
    }
  }
}

/// The short reason (`pressure` or `capacity`) of a retryable refusal in a
/// runner error, or `None` for any other error.
#[must_use]
pub fn refusal(err: &str) -> Option<&'static str> {
  if err.starts_with("resource hold:") {
    Some("pressure")
  } else if err.starts_with("job capacity exhausted") {
    Some("capacity")
  } else {
    None
  }
}

/// The lease, after as many refusals as `wait` allows.
fn acquire_waiting(
  root: &Path,
  request: &LeaseRequest,
  wait: &mut JobWait<'_>,
) -> Result<Ordered, String> {
  let began = Instant::now();
  loop {
    wait.tell(Admission::Attempt);
    let err = match acquire_lease(root, request, SystemTime::now()) {
      Ok(lease) => return Ok(lease),
      Err(err) => err,
    };
    let Some(reason) = refusal(&err) else {
      return Err(err);
    };
    wait.tell(Admission::Refused(reason));
    let waited = began.elapsed();
    if waited.saturating_add(wait.poll) >= wait.bound {
      return if wait.bound.is_zero() {
        Err(err)
      } else {
        Err(format!("{err}; waited {}s", waited.as_secs()))
      };
    }
    // A zero poll with a bound would spin on the lease store and the journal.
    std::thread::sleep(wait.poll.max(Duration::from_millis(50)));
  }
}

/// `runManagedJob(argv, binding, opts)`: `argv` under a job lease on the
/// binding's resource home, refusing at once when the machine is full.
///
/// # Errors
/// A refused admission (`job capacity exhausted (1)`, `resource hold: …`), a
/// run failure, the last heartbeat's failure, or a failed release.
pub fn run_managed_job(
  argv: &[String],
  binding: &ResourceBinding,
  spec: &Spec,
) -> Result<Output, String> {
  run_managed_job_waiting(argv, binding, spec, &mut JobWait::none())
}

/// `argv` under a job lease, waiting for admission as `wait` allows.
///
/// # Errors
/// The last refusal once the bound passes (`job capacity exhausted (1); waited
/// 1800s`), a non-retryable admission error, a run failure, the last
/// heartbeat's failure, or a failed release.
pub fn run_managed_job_waiting(
  argv: &[String],
  binding: &ResourceBinding,
  spec: &Spec,
  wait: &mut JobWait<'_>,
) -> Result<Output, String> {
  let root = binding.root.as_path();
  let request = LeaseRequest {
    kind: LeaseKind::Job,
    key: format!("{}:{}", binding.key, token()),
    state_dir: binding.state_dir.clone(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: Some(binding.worktree.display().to_string()),
  };
  let lease = acquire_waiting(root, &request, wait)?;
  let token = lease_token(&lease)?;
  let log = leased_log(binding, &token, argv)?;
  let failures = Failures::default();
  let observer = log_observer(log, &failures.log);
  let mut group_id = None;
  let mut beating = None;
  let result = GroupGuard::install().and_then(|guard| {
    run_with_chunks(
      &wrapped(argv, spec),
      &mut |pid| {
        group_id = Some(pid);
        guard.arm(pid);
        let running = LeasePatch {
          group_pid: Some(pid),
          stage: Some("running".to_owned()),
          ..LeasePatch::default()
        };
        patch_lease(root, &token, &running)?;
        beating = Some(heartbeat(root, &token, &failures.heartbeat));
        Ok(())
      },
      observer,
    )
    .map_err(|err| format!("{err:?}"))
  });
  finish(root, &token, group_id, beating, &failures)?;
  result
}

#[cfg(test)]
#[path = "tests/jobs_test.rs"]
mod tests;
