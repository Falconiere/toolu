use std::path::Path;

use std::time::Duration;

use toolu_protocol::exit::Exit;

use super::git_identity;
use crate::context::Context;
use crate::lima::Lima;
use crate::lima_stub::Stub;
use crate::state::State;
use crate::step::Step;

fn backend<'a>(stub: &Stub, context: &'a Context) -> Lima<'a> {
  Lima {
    context,
    program: stub.program(),
    state: State::new(&context.env, "toolu-sandbox").waiting(Duration::from_millis(500)),
  }
}

/// The calls that start a command, without the lines a multi-line script adds.
fn commands(stub: &Stub) -> Vec<String> {
  stub
    .calls()
    .into_iter()
    .filter(|line| line.starts_with("--tty=false") || line.starts_with("shell "))
    .collect()
}

#[test]
fn setup_creates_the_vm_from_the_generated_file_provisions_it_and_stamps_it() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let lima = backend(&stub, &context);
  let Step::Report(report) = lima.setup().unwrap() else {
    panic!("setup launched");
  };
  assert_eq!(report.exit, Exit::Success);
  assert_eq!(report.doc["state"], "running");
  assert!(
    report.text.contains("toolu-sandbox-setup.log"),
    "{}",
    report.text
  );
  let yaml = std::fs::read_to_string(lima.state.yaml()).unwrap();
  assert!(
    yaml.contains(&format!("- location: \"{}\"\n", stub.repo().display())),
    "{yaml}"
  );
  assert!(yaml.contains("vmType: \"vz\""), "{yaml}");
  let repo = stub.repo().display().to_string();
  let calls = commands(&stub);
  assert_eq!(
    calls[0],
    format!(
      "--tty=false start --name toolu-sandbox --timeout 30m {}",
      lima.state.yaml().display()
    )
  );
  assert!(
    calls[1].starts_with(&format!(
      "shell --workdir {repo} toolu-sandbox -- bash -euo pipefail -c "
    )),
    "{calls:?}"
  );
  let stamp: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(lima.state.stamp()).unwrap()).unwrap();
  assert_eq!(stamp["toolu"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn setup_boots_a_stopped_vm_and_refuses_one_running_without_this_project() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let lima = backend(&stub, &context);
  stub
    .list(Some(("Running", &[Path::new("/elsewhere")])))
    .unwrap();
  assert_eq!(lima.setup().unwrap_err().exit, Exit::Unavailable);
  assert_eq!(commands(&stub), Vec::<String>::new());
  stub.list(Some(("Stopped", &[]))).unwrap();
  lima.setup().unwrap();
  let calls = commands(&stub);
  assert!(
    calls[0].starts_with("--tty=false edit toolu-sandbox --cpus 10 --memory 16"),
    "{calls:?}"
  );
  assert_eq!(calls[1], "--tty=false start --timeout 30m toolu-sandbox");
  assert!(calls[2].starts_with("shell --workdir "), "{calls:?}");
}

#[test]
fn a_failed_provisioning_names_its_log_and_writes_no_stamp() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let lima = backend(&stub, &context);
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  stub.fail("shell", 7).unwrap();
  let err = lima.setup().unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert!(err.message.contains("exited 7; see "), "{}", err.message);
  assert!(
    err.message.ends_with("toolu-sandbox-setup.log"),
    "{}",
    err.message
  );
  assert!(!lima.state.stamp().exists());
}

#[test]
fn the_git_identity_is_never_empty() {
  let dir = tempfile::tempdir().unwrap();
  let (name, email) = git_identity(dir.path());
  assert!(!name.is_empty() && !email.is_empty());
}
