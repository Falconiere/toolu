//! The host backend: commands run on this machine itself, which on Linux already
//! is the sandbox (a stock kernel with Landlock, no macOS policy). There is no VM
//! to start or stop; `setup` only names the tools this machine still lacks, and
//! `run --clone` keeps its clones under the user's cache directory.

use std::path::PathBuf;

use clap::ArgMatches;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;
use toolu_state::detect::tools::tool_available;

use crate::cli;
use crate::clone::clone_argv;
use crate::context::Context;
use crate::facts::Os;
use crate::present;
use crate::project::slug;
use crate::step::{Failure, Launch, Step};

/// The state every host-backend report names.
const STATE: &str = "host";

/// What the gate and host smoke run, everywhere.
const TOOLS: [&str; 12] = [
  "git",
  "bash",
  "tar",
  "cargo",
  "rustup",
  "node",
  "npm",
  "ast-grep",
  "cargo-nextest",
  "cargo-llvm-cov",
  "cargo-deny",
  "cargo-machete",
];

/// What Linux adds: Codex's and Muse's sandbox needs a system `bwrap`.
const LINUX_TOOLS: [&str; 1] = ["bwrap"];

/// The step of `verb` on the host.
///
/// # Errors
/// A usage failure for an unknown verb or `run --clone` outside a git checkout.
pub(crate) fn step(verb: &str, args: &ArgMatches, context: &Context) -> Result<Step, Failure> {
  match verb {
    "status" => Ok(Step::Report(present::status(context, STATE, None))),
    "setup" => Ok(Step::Report(present::setup(
      context,
      STATE,
      &missing(&context.env, context.facts.os),
    ))),
    "start" | "stop" | "delete" => Ok(Step::Report(present::vm(
      context,
      verb,
      STATE,
      false,
      &format!("the host backend has no VM to {verb}; commands run on this machine"),
    ))),
    "run" => launch(context, &cli::argv(args), cli::set(args, "clone")).map(Step::Launch),
    "shell" => Ok(Step::Launch(Launch {
      argv: vec![
        context.env.get("SHELL").unwrap_or("/bin/sh").to_owned(),
        "-l".to_owned(),
      ],
      cwd: context.project.cwd.clone(),
    })),
    other => Err(Failure::new(
      Exit::Usage,
      format!("`{other}` is not a sandbox verb"),
    )),
  }
}

/// The tools of [`TOOLS`] (and, on Linux, [`LINUX_TOOLS`]) not on `PATH`.
pub(crate) fn missing(env: &Env, os: Os) -> Vec<&'static str> {
  let linux: &[&'static str] = if os == Os::Linux { &LINUX_TOOLS } else { &[] };
  TOOLS
    .iter()
    .chain(linux)
    .copied()
    .filter(|tool| !tool_available(tool, env))
    .collect()
}

/// `command` in this directory, or in this checkout's clone with `clone`.
fn launch(context: &Context, command: &[String], clone: bool) -> Result<Launch, Failure> {
  let project = &context.project;
  if !clone {
    return Ok(Launch {
      argv: command.to_vec(),
      cwd: project.cwd.clone(),
    });
  }
  let (Some(root), Some(rel)) = (project.root.as_deref(), project.rel()) else {
    return Err(Failure::new(
      Exit::Usage,
      format!(
        "--clone needs a git checkout; {} is not in one",
        project.cwd.display()
      ),
    ));
  };
  let dst = cache_root(&context.env).join("work").join(slug(root));
  Ok(Launch {
    argv: clone_argv(root, &dst, &rel, command),
    cwd: project.cwd.clone(),
  })
}

/// `$XDG_CACHE_HOME/toolu-sandbox`, or `~/.cache/toolu-sandbox`.
pub(crate) fn cache_root(env: &Env) -> PathBuf {
  env
    .get("XDG_CACHE_HOME")
    .map_or_else(|| env.home().join(".cache"), PathBuf::from)
    .join("toolu-sandbox")
}

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;
