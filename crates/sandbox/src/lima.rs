//! The Lima backend (macOS's default): one Debian 13 VM per machine that mounts
//! only the projects it runs for, each at its host path. `setup` creates and
//! provisions it; `run` and `shell` boot a stopped VM, check that this project
//! is mounted, and hand the terminal to `limactl shell`. Adding a mount to a
//! running VM needs `start --restart`, which stops every command running in it,
//! so nothing restarts it implicitly.

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::ArgMatches;
use toolu_protocol::exit::Exit;

use crate::cli;
use crate::clone::clone_argv;
use crate::context::Context;
use crate::lima_argv;
use crate::lima_instance::{Instance, Status, find, major_version};
use crate::lima_process::{logged, query};
use crate::mounts::{missing, plan};
use crate::present;
use crate::project::{canonical, slug};
use crate::provision::{GUEST_CACHE, IMAGE_BASE, VmSpec};
use crate::state::State;
use crate::step::{Failure, Launch, Step};

/// How long booting may take, Lima's own wait included.
pub(crate) const BOOT: Duration = Duration::from_mins(35);
pub(crate) const NO_VM: &str = "there is no VM yet; run `toolu sandbox setup`";
/// What `run` says about a VM whose setup never finished.
pub(crate) const NOT_PROVISIONED: &str =
  "the VM is not provisioned (a setup did not finish); run `toolu sandbox setup`";

/// The step of `verb` with the Lima backend. Without a usable `limactl`, `status`
/// reports the backend unavailable and every other verb exits 69.
///
/// # Errors
/// A failure naming what to do next.
pub(crate) fn step(verb: &str, args: &ArgMatches, context: &Context) -> Result<Step, Failure> {
  let lima = match Lima::new(context) {
    Ok(lima) => lima,
    Err(why) if verb == "status" && why.exit == Exit::Unavailable => {
      return Ok(Step::Report(present::status(
        context,
        "unavailable",
        Some(&why.message),
      )));
    }
    Err(why) => return Err(why),
  };
  match verb {
    "status" => lima.status(),
    "setup" => lima.setup(),
    "start" => lima.start(cli::set(args, "restart")),
    "stop" => lima.stop(),
    "delete" => lima.delete(),
    "run" => lima.launch(&cli::argv(args), cli::set(args, "clone")),
    "shell" => lima.launch(&["bash".to_owned(), "--login".to_owned()], false),
    other => Err(Failure::new(
      Exit::Usage,
      format!("`{other}` is not a sandbox verb"),
    )),
  }
}

/// The Lima backend for one context.
pub(crate) struct Lima<'a> {
  pub(crate) context: &'a Context,
  /// The `limactl` found on the context's `PATH`.
  pub(crate) program: PathBuf,
  pub(crate) state: State,
}

impl<'a> Lima<'a> {
  fn new(context: &'a Context) -> Result<Lima<'a>, Failure> {
    let program = lima_argv::find(&context.env).ok_or_else(|| {
      Failure::new(
        Exit::Unavailable,
        "limactl is not on PATH; install Lima 2: brew install lima",
      )
    })?;
    let text = query(&lima_argv::version(&program))?;
    if major_version(&text).is_none_or(|major| major < 2) {
      return Err(Failure::new(
        Exit::Unavailable,
        format!(
          "toolu sandbox needs Lima 2 or newer, not `{}`; brew upgrade lima",
          text.trim()
        ),
      ));
    }
    Ok(Lima {
      program,
      state: State::new(&context.env, &context.settings.name),
      context,
    })
  }

  pub(crate) fn name(&self) -> &str {
    &self.context.settings.name
  }

  pub(crate) fn instance(&self) -> Result<Option<Instance>, Failure> {
    find(&query(&lima_argv::list(&self.program))?, self.name())
      .map_err(|why| Failure::new(Exit::Failure, why))
  }

  /// The VM the settings and this project ask for.
  pub(crate) fn spec(&self) -> VmSpec {
    let settings = &self.context.settings;
    let extra = settings.mounts.iter().map(|mount| canonical(mount));
    VmSpec {
      cpus: settings.cpus,
      memory_gib: settings.memory_gib,
      disk_gib: settings.disk_gib,
      base: settings
        .template
        .clone()
        .unwrap_or_else(|| IMAGE_BASE.to_owned()),
      mounts: plan(self.context.project.mounts().into_iter().chain(extra)),
    }
  }

  /// This project's directories that `instance` does not mount.
  pub(crate) fn unmounted(&self, instance: &Instance) -> Vec<PathBuf> {
    missing(&instance.mounts, &self.context.project.mounts())
  }

  /// Ok when `instance` mounts this project, else what to do about it.
  ///
  /// # Errors
  /// Exit 69 naming the directories the VM lacks.
  pub(crate) fn mounted(&self, instance: &Instance) -> Result<(), Failure> {
    let unmounted = self.unmounted(instance);
    if unmounted.is_empty() {
      Ok(())
    } else {
      Err(not_mounted(&unmounted))
    }
  }
}

/// Exit 69 naming the project directories the running VM does not mount.
fn not_mounted(paths: &[PathBuf]) -> Failure {
  let paths: Vec<String> = paths
    .iter()
    .map(|path| path.display().to_string())
    .collect();
  Failure::new(
    Exit::Unavailable,
    format!(
      "the running VM does not mount {}; run `toolu sandbox start --restart` (it stops every command running in the VM), or add a parent directory to sandbox.mounts",
      paths.join(", ")
    ),
  )
}

impl Lima<'_> {
  /// Apply the settings and mounts to the stopped VM and boot it.
  pub(crate) fn boot(&self) -> Result<(), Failure> {
    query(&lima_argv::edit(&self.program, self.name(), &self.spec()))?;
    logged(
      &lima_argv::start(&self.program, self.name()),
      &self.context.project.cwd,
      &self.state.log("start"),
      BOOT,
    )
  }

  /// Refuse a project directory that holds the user's home directory, which the
  /// VM would mount writable, unless `sandbox.mounts` lists it on purpose.
  ///
  /// # Errors
  /// Exit 64 naming the directory.
  pub(crate) fn guard_project(&self) -> Result<(), Failure> {
    let primary = self.context.project.primary();
    let listed = (self.context.settings.mounts.iter()).any(|mount| canonical(mount) == primary);
    if canonical(&self.context.env.home()).starts_with(&primary) && !listed {
      return Err(Failure::new(
        Exit::Usage,
        format!(
          "{} holds your home directory, which the VM would mount writable; run from a project directory, or list it in sandbox.mounts to allow it",
          primary.display()
        ),
      ));
    }
    Ok(())
  }

  /// `command` in the VM at this directory, or in this checkout's clone there.
  fn launch(&self, command: &[String], clone: bool) -> Result<Step, Failure> {
    self.guard_project()?;
    let project = &self.context.project;
    let argv = if clone {
      let (Some(root), Some(rel)) = (project.root.as_deref(), project.rel()) else {
        return Err(Failure::new(
          Exit::Usage,
          format!(
            "--clone needs a git checkout; {} is not in one",
            project.cwd.display()
          ),
        ));
      };
      let dst = Path::new(GUEST_CACHE).join("work").join(slug(root));
      let mut inner = lima_argv::guest_env(&slug(root), false);
      inner.extend(clone_argv(root, &dst, &rel, command));
      lima_argv::shell(&self.program, self.name(), Path::new("/"), &inner)
    } else {
      let base = project.root.as_deref().unwrap_or(&project.cwd);
      let mut inner = lima_argv::guest_env(&slug(base), true);
      inner.extend(command.iter().cloned());
      lima_argv::shell(&self.program, self.name(), &project.cwd, &inner)
    };
    self.ready()?;
    Ok(Step::Launch(Launch {
      argv,
      cwd: project.cwd.clone(),
    }))
  }

  /// A running VM that mounts this project, booting a stopped one first.
  fn ready(&self) -> Result<(), Failure> {
    let instance = self.existing()?;
    if !self.state.stamp().is_file() {
      return Err(Failure::new(Exit::Unavailable, NOT_PROVISIONED));
    }
    if instance.status == Status::Running {
      return self.mounted(&instance);
    }
    let _lock = self.state.lock()?;
    // Another command may have booted the VM while this one went for the lock,
    // and `limactl edit` refuses a running VM.
    let instance = self.existing()?;
    if instance.status == Status::Running {
      return self.mounted(&instance);
    }
    self.boot()
  }

  /// The VM, or exit 69 when there is none yet.
  fn existing(&self) -> Result<Instance, Failure> {
    self
      .instance()?
      .ok_or_else(|| Failure::new(Exit::Unavailable, NO_VM))
  }
}

#[cfg(test)]
#[path = "tests/lima_test.rs"]
mod tests;
