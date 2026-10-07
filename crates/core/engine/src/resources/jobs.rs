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
use std::time::{Duration, SystemTime};

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::guard::GroupGuard;
use toolu_runtime::process::{Output, Spec, group, run_with};
use toolu_state::lock::token;
use toolu_state::time::iso_millis;

use super::binding::ResourceBinding;
use super::leases::{
  LeaseKind, LeasePatch, LeaseRequest, acquire_lease, patch_lease, release_lease,
};

/// How often a running job refreshes its lease.
const HEARTBEAT: Duration = Duration::from_secs(30);
/// The shell that waits for the go line, then becomes the job.
const GO_SHELL: &str = "IFS= read -r ack && [ \"$ack\" = toolu-go ] && exec \"$@\"";

/// The latest heartbeat failure, cleared by the next success.
type Failure = Arc<Mutex<Option<String>>>;

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

/// `runManagedJob(argv, binding, opts)`: `argv` under a job lease on the
/// binding's resource home.
///
/// # Errors
/// A refused admission (`job capacity exhausted (1)`, `resource hold: …`), a
/// run failure, the last heartbeat's failure, or a failed release.
pub fn run_managed_job(
  argv: &[String],
  binding: &ResourceBinding,
  spec: &Spec,
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
  let lease = acquire_lease(root, &request, SystemTime::now())?;
  let token = lease_token(&lease)?;
  let failure: Failure = Arc::new(Mutex::new(None));
  let mut group_id = None;
  let mut beating = None;
  let result = GroupGuard::install().and_then(|guard| {
    run_with(&wrapped(argv, spec), &mut |pid| {
      group_id = Some(pid);
      guard.arm(pid);
      let running = LeasePatch {
        group_pid: Some(pid),
        stage: Some("running".to_owned()),
        heartbeat_at: None,
      };
      patch_lease(root, &token, &running)?;
      beating = Some(heartbeat(root, &token, &failure));
      Ok(())
    })
    .map_err(|err| format!("{err:?}"))
  });
  if let Some((stop, handle)) = beating {
    drop(stop);
    let _joined = handle.join();
  }
  settle(root, &token, group_id)?;
  if let Some(failed) = failure
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .take()
  {
    return Err(failed);
  }
  result
}

#[cfg(test)]
#[path = "tests/jobs_test.rs"]
mod tests;
