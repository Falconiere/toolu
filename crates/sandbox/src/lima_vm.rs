//! The Lima backend's VM verbs: `status`, `start`, `stop`, `delete` and `prune`.

use toolu_protocol::exit::Exit;

use crate::lima::{Lima, NO_VM};
use crate::lima_argv;
use crate::lima_instance::Status;
use crate::lima_process::query;
use crate::present;
use crate::step::{Failure, Step};

impl Lima<'_> {
  pub(super) fn status(&self) -> Result<Step, Failure> {
    let report = match self.instance()? {
      None => present::status(self.context, "absent", Some(NO_VM)),
      Some(instance) => {
        let reason = (instance.status == Status::Running)
          .then(|| self.mounted(&instance).err())
          .flatten()
          .map(|failure| failure.message);
        present::status(self.context, instance.status.name(), reason.as_deref())
      }
    };
    Ok(Step::Report(report))
  }

  pub(super) fn start(&self, restart: bool) -> Result<Step, Failure> {
    self.guard_project()?;
    let _lock = self.state.lock()?;
    let instance = self
      .instance()?
      .ok_or_else(|| Failure::new(Exit::Unavailable, NO_VM))?;
    if instance.status == Status::Running {
      if !restart {
        self.mounted(&instance)?;
        return Ok(self.vm("start", "running", false, "the VM is already running"));
      }
      query(&lima_argv::stop(&self.program, self.name()))?;
    }
    self.boot()?;
    Ok(self.vm("start", "running", true, "the VM is running"))
  }

  pub(super) fn stop(&self) -> Result<Step, Failure> {
    let _lock = self.state.lock()?;
    match self.instance()? {
      Some(instance) if instance.status == Status::Running => {
        query(&lima_argv::stop(&self.program, self.name()))?;
        Ok(self.vm("stop", "stopped", true, "the VM stopped"))
      }
      Some(instance) => Ok(self.vm(
        "stop",
        instance.status.name(),
        false,
        "the VM is not running",
      )),
      None => Ok(self.vm("stop", "absent", false, "there is no VM to stop")),
    }
  }

  pub(super) fn delete(&self) -> Result<Step, Failure> {
    let _lock = self.state.lock()?;
    if self.instance()?.is_none() {
      return Ok(self.vm("delete", "absent", false, "there is no VM to delete"));
    }
    query(&lima_argv::delete(&self.program, self.name()))?;
    self.state.forget();
    Ok(self.vm("delete", "absent", true, "the VM and its disk are deleted"))
  }

  /// The orphan clones on the VM's disk, removed with `apply`; a stopped VM is
  /// reported as such and not booted.
  pub(super) fn prune(&self, apply: bool) -> Result<Step, Failure> {
    let state = match self.instance()? {
      Some(instance) if instance.status == Status::Running => "running",
      Some(instance) => {
        let state = instance.status.name();
        return Ok(Step::Report(crate::prune::report(
          self.context,
          state,
          apply,
          (&[], &[]),
        )));
      }
      None => {
        return Ok(Step::Report(crate::prune::report(
          self.context,
          "absent",
          apply,
          (&[], &[]),
        )));
      }
    };
    let run = |argv: &[String]| {
      query(&lima_argv::shell(
        &self.program,
        self.name(),
        std::path::Path::new("/"),
        argv,
      ))
    };
    let cache = std::path::Path::new(crate::provision::GUEST_CACHE);
    let (removed, kept) = crate::prune::prune_with(run, cache, apply)?;
    Ok(Step::Report(crate::prune::report(
      self.context,
      state,
      apply,
      (&removed, &kept),
    )))
  }

  pub(super) fn vm(&self, verb: &str, state: &str, changed: bool, message: &str) -> Step {
    Step::Report(present::vm(self.context, verb, state, changed, message))
  }
}

#[cfg(test)]
#[path = "tests/lima_vm_test.rs"]
mod tests;
