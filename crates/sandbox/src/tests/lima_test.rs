use std::path::Path;

use toolu_protocol::exit::Exit;

use super::step;
use crate::cli::command;
use crate::context::Context;
use crate::lima_stub::Stub;
use crate::step::{Failure, Launch, Step};

fn verb(context: &Context, args: &[&str]) -> Result<Step, Failure> {
  let matches = parsed(args);
  let (name, verb) = named(&matches);
  step(name, verb, context)
}

fn parsed(args: &[&str]) -> clap::ArgMatches {
  command().try_get_matches_from(args).unwrap()
}

fn named(matches: &clap::ArgMatches) -> (&str, &clap::ArgMatches) {
  matches.subcommand().unwrap()
}

fn report(step: Result<Step, Failure>) -> serde_json::Value {
  match step.unwrap() {
    Step::Report(report) => report.doc,
    Step::Launch(launch) => panic!("launched {launch:?}"),
  }
}

fn launched(step: Result<Step, Failure>) -> Launch {
  match step.unwrap() {
    Step::Launch(launch) => launch,
    Step::Report(report) => panic!("reported {report:?}"),
  }
}

#[test]
fn without_limactl_or_with_an_old_one_status_says_why_and_the_rest_exit_69() {
  let stub = Stub::new("2.2.1").unwrap();
  let mut context = stub.context();
  context.env = context.env.with("PATH", "/usr/bin:/bin");
  let doc = report(verb(&context, &["sandbox", "status"]));
  assert_eq!(doc["state"], "unavailable");
  assert_eq!(
    doc["reason"],
    "limactl is not on PATH; install Lima 2: brew install lima"
  );
  assert_eq!(
    verb(&context, &["sandbox", "stop"]).unwrap_err().exit,
    Exit::Unavailable
  );
  let old = Stub::new("1.0.7").unwrap();
  let doc = report(verb(&old.context(), &["sandbox", "status"]));
  assert_eq!(
    doc["reason"],
    "toolu sandbox needs Lima 2 or newer, not `limactl version 1.0.7`; brew upgrade lima"
  );
}

#[test]
fn status_reports_absent_running_and_an_unmounted_project() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let absent = report(verb(&context, &["sandbox", "status"]));
  assert_eq!(
    (absent["state"].as_str(), absent["reason"].as_str()),
    (Some("absent"), Some(super::NO_VM))
  );
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  let running = report(verb(&context, &["sandbox", "status"]));
  assert_eq!(running["state"], "running");
  assert_eq!(running["reason"], serde_json::Value::Null);
  stub
    .list(Some(("Running", &[Path::new("/elsewhere")])))
    .unwrap();
  let unmounted = report(verb(&context, &["sandbox", "status"]));
  let reason = unmounted["reason"].as_str().unwrap();
  assert!(
    reason.starts_with("the running VM does not mount "),
    "{reason}"
  );
  assert!(reason.contains("toolu sandbox start --restart"), "{reason}");
}

#[test]
fn run_hands_the_terminal_to_limactl_shell_in_this_directory() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  let launch = launched(verb(&context, &["sandbox", "run", "--", "cargo", "test"]));
  let program = stub.program().display().to_string();
  let workdir = stub.repo().join("sub").display().to_string();
  let slug = crate::project::slug(&stub.repo());
  assert_eq!(launch.cwd, stub.repo().join("sub"));
  assert_eq!(
    launch.argv,
    [
      program.as_str(),
      "shell",
      "--workdir",
      &workdir,
      "toolu-sandbox",
      "--",
      "env",
      "TMPDIR=/var/cache/toolu-sandbox/tmp",
      &format!("CARGO_TARGET_DIR=/var/cache/toolu-sandbox/target/{slug}"),
      "cargo",
      "test"
    ]
  );
  let shell = launched(verb(&context, &["sandbox", "shell"]));
  assert_eq!(shell.argv[shell.argv.len() - 2..], ["bash", "--login"]);
  assert_eq!(stub.calls(), Vec::<String>::new());
}

#[test]
fn run_clone_runs_the_clone_script_from_the_guest_root() {
  let stub = Stub::new("2.2.1").unwrap();
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  let launch = launched(verb(
    &stub.context(),
    &["sandbox", "run", "--clone", "--", "make"],
  ));
  let slug = crate::project::slug(&stub.repo());
  assert_eq!(launch.argv[3], "/");
  assert_eq!(
    &launch.argv[6..8],
    ["env", "TMPDIR=/var/cache/toolu-sandbox/tmp"]
  );
  assert_eq!(launch.argv[12], crate::clone::CLONE);
  let tail = &launch.argv[13..];
  assert_eq!(tail[1], stub.repo().display().to_string());
  assert_eq!(tail[2], format!("/var/cache/toolu-sandbox/work/{slug}"));
  assert_eq!(&tail[3..], ["sub", "make"]);
  let mut outside = stub.context();
  outside.project.root = None;
  let err = verb(&outside, &["sandbox", "run", "--clone", "make"]).unwrap_err();
  assert_eq!(err.exit, Exit::Usage);
}

#[test]
fn run_boots_a_stopped_vm_and_refuses_an_unmounted_or_missing_one() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let none = verb(&context, &["sandbox", "run", "true"]).unwrap_err();
  assert_eq!(
    (none.exit, none.message.as_str()),
    (Exit::Unavailable, super::NO_VM)
  );
  stub
    .list(Some(("Running", &[Path::new("/elsewhere")])))
    .unwrap();
  let refused = verb(&context, &["sandbox", "run", "true"]).unwrap_err();
  assert_eq!(refused.exit, Exit::Unavailable);
  assert_eq!(stub.calls(), Vec::<String>::new());
  stub.list(Some(("Stopped", &[]))).unwrap();
  launched(verb(&context, &["sandbox", "run", "true"]));
  let repo = stub.repo().display().to_string();
  assert_eq!(
    stub.calls(),
    [
      format!("--tty=false edit toolu-sandbox --cpus 10 --memory 16 --mount-only {repo}:w"),
      "--tty=false start --timeout 30m toolu-sandbox".to_owned(),
    ]
  );
}

#[test]
fn run_does_not_boot_a_vm_another_command_started_while_it_waited_for_the_lock() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  stub.list_first(Some(("Stopped", &[]))).unwrap();
  launched(verb(&context, &["sandbox", "run", "true"]));
  assert_eq!(stub.calls(), Vec::<String>::new());
  stub
    .list(Some(("Running", &[Path::new("/elsewhere")])))
    .unwrap();
  stub.list_first(Some(("Stopped", &[]))).unwrap();
  let refused = verb(&context, &["sandbox", "run", "true"]).unwrap_err();
  assert_eq!(refused.exit, Exit::Unavailable);
  assert_eq!(stub.calls(), Vec::<String>::new());
}

#[test]
fn a_failed_boot_names_its_log_and_an_unknown_verb_is_usage() {
  let stub = Stub::new("2.2.1").unwrap();
  stub.list(Some(("Stopped", &[]))).unwrap();
  stub.fail("start", 5).unwrap();
  let err = verb(&stub.context(), &["sandbox", "run", "true"]).unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert!(err.message.contains("exited 5; see "), "{}", err.message);
  assert!(
    err.message.ends_with("toolu-sandbox-start.log"),
    "{}",
    err.message
  );
  let bogus = step("bogus", &clap::ArgMatches::default(), &stub.context()).unwrap_err();
  assert_eq!(bogus.exit, Exit::Usage);
}

#[test]
fn an_unprovisioned_vm_is_refused_before_anything_runs() {
  let stub = Stub::new("2.2.1").unwrap();
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  stub.unprovision().unwrap();
  for args in [["sandbox", "run", "true"], ["sandbox", "shell", "-"]] {
    let args = &args[..if args[1] == "shell" { 2 } else { 3 }];
    let err = verb(&stub.context(), args).unwrap_err();
    assert_eq!(
      (err.exit, err.message.as_str()),
      (Exit::Unavailable, super::NOT_PROVISIONED)
    );
  }
  assert_eq!(stub.calls(), Vec::<String>::new());
}

#[test]
fn a_project_that_holds_the_home_directory_is_refused_unless_listed() {
  let stub = Stub::new("2.2.1").unwrap();
  stub
    .list(Some(("Running", &[stub.root.as_path()])))
    .unwrap();
  let mut context = stub.context();
  // HOME is the stub's root, so a project rooted there holds it.
  context.project.root = Some(stub.root.clone());
  context.project.cwd = stub.root.clone();
  for args in [
    &["sandbox", "run", "true"][..],
    &["sandbox", "start"],
    &["sandbox", "setup"],
  ] {
    let err = verb(&context, args).unwrap_err();
    assert_eq!(err.exit, Exit::Usage, "{args:?}");
    assert!(
      err.message.contains("holds your home directory"),
      "{}",
      err.message
    );
  }
  assert_eq!(stub.calls(), Vec::<String>::new());
  context.settings.mounts = vec![stub.root.clone()];
  launched(verb(&context, &["sandbox", "run", "true"]));
  // Outside git the directory itself is the project.
  let mut outside = stub.context();
  outside.project.root = None;
  outside.project.cwd = stub.root;
  assert_eq!(
    verb(&outside, &["sandbox", "run", "true"])
      .unwrap_err()
      .exit,
    Exit::Usage
  );
}
