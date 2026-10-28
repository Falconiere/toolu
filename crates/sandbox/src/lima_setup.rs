//! The Lima backend's `setup`: create the VM from the generated Lima file (or
//! boot the stopped one), run the provisioning script in it, and stamp the
//! result. Both long steps log to files under the state directory.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::lima::{BOOT, Lima};
use crate::lima_argv;
use crate::lima_instance::Status;
use crate::lima_process::{logged, query};
use crate::present;
use crate::provision::{GUEST_CACHE, PROVISION, lima_yaml};
use crate::state::write;
use crate::step::{Failure, Step};

/// The toolu version provisioned into the guest.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// How long provisioning may take on a cold VM and a slow network.
const PROVISIONING: Duration = Duration::from_mins(90);

impl Lima<'_> {
  pub(super) fn setup(&self) -> Result<Step, Failure> {
    self.guard_project()?;
    let _lock = self.state.lock()?;
    let existing = self.instance()?;
    self.state.clear_stamp();
    match existing {
      None => self.create()?,
      Some(instance) if instance.status == Status::Running => self.mounted(&instance)?,
      Some(_) => self.boot()?,
    }
    self.provision()?;
    let mut report = present::setup(self.context, Status::Running.name(), &[]);
    report.text = format!(
      "{}\n  logs: {} and {}",
      report.text,
      self.state.log("start").display(),
      self.state.log("setup").display()
    );
    Ok(Step::Report(report))
  }

  pub(super) fn create(&self) -> Result<(), Failure> {
    let yaml = self.state.yaml();
    let text = lima_yaml(&self.spec(), self.context.facts.os, VERSION);
    write(&yaml, &text)?;
    logged(
      &lima_argv::create(&self.program, self.name(), &yaml),
      &self.context.project.cwd,
      &self.state.log("start"),
      BOOT,
    )
  }

  pub(super) fn provision(&self) -> Result<(), Failure> {
    let project = &self.context.project;
    let workdir = project.root.as_deref().unwrap_or(&project.cwd);
    let (name, email) = git_identity(workdir);
    let script = [
      "bash",
      "-euo",
      "pipefail",
      "-c",
      PROVISION,
      "toolu-sandbox-setup",
      VERSION,
      GUEST_CACHE,
      &name,
      &email,
    ]
    .map(str::to_owned);
    let argv = lima_argv::shell(&self.program, self.name(), workdir, &script);
    logged(&argv, workdir, &self.state.log("setup"), PROVISIONING)?;
    let at = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_or(0, |since| since.as_secs());
    let stamp = json!({ "toolu": VERSION, "provisioned_at": at });
    write(&self.state.stamp(), &stamp.to_string())
  }
}

/// The host's git name and email for commits made in the guest.
fn git_identity(cwd: &Path) -> (String, String) {
  let read = |key: &str| {
    let argv = ["git", "-C", &cwd.to_string_lossy(), "config", "--get", key].map(str::to_owned);
    query(&argv)
      .ok()
      .map(|text| text.trim().to_owned())
      .filter(|text| !text.is_empty())
  };
  (
    read("user.name").unwrap_or_else(|| "toolu sandbox".to_owned()),
    read("user.email").unwrap_or_else(|| "sandbox@toolu.invalid".to_owned()),
  )
}

#[cfg(test)]
#[path = "tests/lima_setup_test.rs"]
mod tests;
