use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{LINUX_TOOLS, TOOLS, cache_root, missing, step};
use crate::cli::command;
use crate::clone::CLONE;
use crate::context::Context;
use crate::facts::{Facts, Os};
use crate::project::{Project, slug};
use crate::settings::{Backend, Resolved};
use crate::step::{Launch, Step};

fn context(env: Env, root: Option<&str>) -> Context {
  Context {
    env,
    facts: Facts {
      os: Os::Linux,
      cores: 4,
      memory_gib: 8,
    },
    settings: Resolved {
      backend: Backend::Host,
      name: "toolu-sandbox".to_owned(),
      cpus: 2,
      memory_gib: 4,
      disk_gib: 100,
      template: None,
      mounts: Vec::new(),
    },
    project: Project {
      cwd: PathBuf::from("/repo/sub"),
      root: root.map(PathBuf::from),
      common: None,
    },
  }
}

fn home() -> Env {
  Env::from_pairs([("HOME", "/home/dev"), ("SHELL", "/bin/zsh")])
}

fn parsed(args: &[&str]) -> clap::ArgMatches {
  command().try_get_matches_from(args).unwrap()
}

fn verb(context: &Context, args: &[&str]) -> Result<Step, crate::step::Failure> {
  let matches = parsed(args);
  let (name, verb) = named(&matches);
  step(name, verb, context)
}

fn named(matches: &clap::ArgMatches) -> (&str, &clap::ArgMatches) {
  matches.subcommand().unwrap()
}

fn launched(step: Result<Step, crate::step::Failure>) -> Launch {
  match step.unwrap() {
    Step::Launch(launch) => launch,
    Step::Report(report) => panic!("expected a launch, got {report:?}"),
  }
}

#[test]
fn run_starts_the_command_in_this_directory() {
  let context = context(home(), Some("/repo"));
  let launch = launched(verb(&context, &["sandbox", "run", "--", "make", "-j4"]));
  assert_eq!(
    launch,
    Launch {
      argv: vec!["make".to_owned(), "-j4".to_owned()],
      cwd: PathBuf::from("/repo/sub"),
    }
  );
}

#[test]
fn run_clone_runs_in_a_clone_under_the_cache() {
  let context = context(home(), Some("/repo"));
  let launch = launched(verb(&context, &["sandbox", "run", "--clone", "--", "make"]));
  let dst = PathBuf::from("/home/dev/.cache/toolu-sandbox/work").join(slug(Path::new("/repo")));
  assert_eq!(launch.argv[4], CLONE);
  assert_eq!(
    &launch.argv[5..],
    [
      "toolu-sandbox-clone",
      "/repo",
      dst.to_str().unwrap(),
      "sub",
      "make"
    ]
  );
  let outside = verb(
    &self::context(home(), None),
    &["sandbox", "run", "--clone", "--", "make"],
  );
  let err = outside.unwrap_err();
  assert_eq!(err.exit, Exit::Usage);
  assert_eq!(
    err.message,
    "--clone needs a git checkout; /repo/sub is not in one"
  );
}

#[test]
fn shell_is_the_users_login_shell_or_sh() {
  let zsh = launched(verb(&context(home(), None), &["sandbox", "shell"]));
  assert_eq!(zsh.argv, ["/bin/zsh", "-l"]);
  let bare = Env::from_pairs([("HOME", "/home/dev")]);
  let sh = launched(verb(&context(bare, None), &["sandbox", "shell"]));
  assert_eq!(sh.argv, ["/bin/sh", "-l"]);
}

#[test]
fn the_vm_verbs_change_nothing_and_status_is_host() {
  let context = context(home(), None);
  for args in [
    &["sandbox", "start", "--restart"][..],
    &["sandbox", "stop"],
    &["sandbox", "delete", "--yes"],
  ] {
    let Step::Report(report) = verb(&context, args).unwrap() else {
      panic!("{args:?} launched");
    };
    assert_eq!(report.exit, Exit::Success);
    assert_eq!(report.doc["changed"], false);
    assert_eq!(report.doc["state"], "host");
  }
  let Step::Report(status) = verb(&context, &["sandbox", "status"]).unwrap() else {
    panic!("status launched");
  };
  assert_eq!(status.doc["state"], "host");
  let err = step("bogus", &clap::ArgMatches::default(), &context).unwrap_err();
  assert_eq!(err.exit, Exit::Usage);
}

/// A `PATH` of one fresh directory holding every tool but `lacking`.
fn path_without(dir: &Path, lacking: &[&str]) -> Env {
  std::fs::create_dir_all(dir).unwrap();
  for tool in TOOLS.iter().chain(&LINUX_TOOLS) {
    if lacking.contains(tool) {
      continue;
    }
    let file = dir.join(tool);
    std::fs::write(&file, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
  }
  Env::from_pairs([("PATH", dir.to_str().unwrap()), ("HOME", "/home/dev")])
}

#[test]
fn setup_and_missing_name_the_tools_path_lacks() {
  // `tool_available` caches per PATH string, so each case gets its own directory.
  let dir = tempfile::tempdir().unwrap();
  let full = path_without(&dir.path().join("full"), &[]);
  assert_eq!(missing(&full, Os::Linux), Vec::<&str>::new());
  let one = path_without(&dir.path().join("one"), &["node"]);
  assert_eq!(missing(&one, Os::Linux), ["node"]);
  let env = path_without(&dir.path().join("two"), &["node", "bwrap"]);
  assert_eq!(missing(&env, Os::Macos), ["node"]);
  assert_eq!(missing(&env, Os::Linux), ["node", "bwrap"]);
  let Step::Report(report) = verb(&context(env, None), &["sandbox", "setup"]).unwrap() else {
    panic!("setup launched");
  };
  assert_eq!(report.exit, Exit::Unavailable);
  assert_eq!(report.doc["missing"], serde_json::json!(["node", "bwrap"]));
}

#[test]
fn the_cache_root_honours_xdg() {
  assert_eq!(
    cache_root(&home()),
    PathBuf::from("/home/dev/.cache/toolu-sandbox")
  );
  let xdg = Env::from_pairs([("HOME", "/home/dev"), ("XDG_CACHE_HOME", "/c")]);
  assert_eq!(cache_root(&xdg), PathBuf::from("/c/toolu-sandbox"));
}
