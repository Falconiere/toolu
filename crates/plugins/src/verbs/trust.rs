//! Approval after `install` and `update` (#549). Muse keeps a plugin's hooks
//! and agent definitions inert until `muse plugins approve` trusts them, and a
//! changed definition needs trusting again. Under `--approve` toolu approves
//! what is pending; otherwise the step names the command, and the run still
//! succeeds. A host without an approval step has no inspect command, so its
//! steps pass through unchanged.

use crate::args::Approval;
use crate::host::muse;
use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::Driver;

/// Settles the trust of every plugin that is on the host after the verb.
pub(crate) fn settle(driver: &Driver, steps: Vec<Step>) -> Vec<Step> {
  steps.into_iter().map(|step| one(driver, step)).collect()
}

fn failed(step: Step, detail: String) -> Step {
  Step {
    status: Status::Failed,
    detail,
    ..step
  }
}

fn one(driver: &Driver, step: Step) -> Step {
  let present = matches!(
    step.status,
    Status::Installed | Status::Already | Status::Updated | Status::Current
  );
  let adapter = driver.adapter;
  let (Some(inspect), Some(approve)) = (adapter.inspect(&step.name), adapter.approve(&step.name))
  else {
    return step;
  };
  if !present {
    return step;
  }
  let ran = driver.run(&inspect);
  let pending = if ran.ok() {
    muse::pending(&ran.stdout)
  } else {
    Err(first_line(&ran.stderr, &format!("exited {}", ran.code)))
  };
  match pending {
    Err(detail) => {
      let detail = format!(
        "{}, but `{}` failed: {detail}",
        step.detail,
        inspect.join(" ")
      );
      failed(step, detail)
    }
    Ok(false) => step,
    Ok(true) if driver.approval == Approval::Approve => {
      let ran = driver.run(&approve);
      if ran.ok() {
        let detail = format!("{}; approved", step.detail);
        return Step { detail, ..step };
      }
      let why = first_line(&ran.stderr, &format!("exited {}", ran.code));
      let detail = format!("{}, but `{}` failed: {why}", step.detail, approve.join(" "));
      failed(step, detail)
    }
    Ok(true) => {
      let detail = format!(
        "{}; awaiting approval: muse plugins approve {}",
        step.detail, step.name
      );
      Step { detail, ..step }
    }
  }
}

#[cfg(test)]
#[path = "tests/trust_test.rs"]
mod tests;
