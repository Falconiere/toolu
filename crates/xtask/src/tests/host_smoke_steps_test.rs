use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::{Driver, Prompts, Run, Step, allowed, commit, deny, push};
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_project::{create, create_push};

/// A driver whose "host CLI" is a shell script that plays back a fixed transcript.
struct Script {
  exe: PathBuf,
  stages: bool,
}

impl Driver for Script {
  fn name(&self) -> &'static str {
    "claude"
  }
  fn argv(&self, step: &Step<'_>, project: &Path) -> Vec<String> {
    let exe = self.exe.display().to_string();
    vec![exe, step.name.to_owned(), project.display().to_string()]
  }
  fn prompts(&self) -> Prompts {
    Prompts {
      allowed: "pwd",
      commit: "commit",
      push: "push",
      stages_first: self.stages,
    }
  }
  fn prepare(&self, _run: &Run<'_>, _project: &Path) -> Result<(), String> {
    Ok(())
  }
  fn edit(&self, _run: &Run<'_>, _project: &Path) -> Result<(), String> {
    Err("no edit".to_owned())
  }
}

fn env(base: &Path) -> ChildEnv {
  let path = std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin"));
  ChildEnv::new(Path::new("/x/toolu"), &path, &base.join("shims")).unwrap()
}

fn host(dir: &Path, body: &str, stages: bool) -> Script {
  let exe = dir.join("host");
  std::fs::write(&exe, format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
  Script { exe, stages }
}

#[test]
fn deny_and_allowed_judge_the_transcript_and_keep_it_in_the_evidence() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let run = Run {
    root: root.path(),
    env: &env,
  };
  let project = create(root.path(), "p", "claude", &env).unwrap();
  let script = host(
    root.path(),
    "case $1 in deny) echo \"Blocked by gates.protectedFiles.mode='block'\";; *) cat \"$2/.toolu-smoke-nonce\";; esac",
    false,
  );
  deny(&script, &run, &project).unwrap();
  allowed(&script, &run, &project).unwrap();
  assert!(root.path().join("claude-deny.jsonl").is_file());
  let silent = host(root.path(), "echo nothing", false);
  assert_eq!(
    deny(&silent, &run, &project).unwrap_err(),
    "no live deny evidence"
  );
  let typed = host(root.path(), "echo \"toolu-smoke-cwd:$2\"", false);
  assert_eq!(
    allowed(&typed, &run, &project).unwrap_err(),
    "the clean control command did not run"
  );
  let init_only = host(root.path(), "echo \"cwd $2\"", false);
  assert_eq!(
    allowed(&init_only, &run, &project).unwrap_err(),
    "the clean control command did not run"
  );
}

#[test]
fn commit_needs_the_quality_denial_and_an_unmoved_head() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let run = Run {
    root: root.path(),
    env: &env,
  };
  let project = create(root.path(), "p", "claude", &env).unwrap();
  let denied = host(root.path(), "echo 'BLOCKED: quality gate failing'", true);
  commit(&denied, &run, &project).unwrap();
  let moved = host(
    root.path(),
    "echo 'quality gate failing'; cd \"$2\" && git commit -q --allow-empty -m x",
    true,
  );
  assert_eq!(
    commit(&moved, &run, &project).unwrap_err(),
    "the commit changed HEAD"
  );
  let allowed_commit = host(root.path(), "echo committed", false);
  assert!(commit(&allowed_commit, &run, &project).is_err());
}

#[test]
fn push_needs_the_review_denial_and_an_untouched_remote() {
  let root = tempfile::tempdir().unwrap();
  let env = env(root.path());
  let run = Run {
    root: root.path(),
    env: &env,
  };
  let pushed = create_push(root.path(), "claude", &env).unwrap();
  let blocked = host(
    root.path(),
    "echo 'Code review required before push (diff SHA abc)'",
    false,
  );
  push(&blocked, &run, &pushed).unwrap();
  let leaked = host(
    root.path(),
    "echo 'Code review required before push'; cd \"$2\" && git push -q origin HEAD:refs/heads/feat/smoke",
    false,
  );
  assert_eq!(
    push(&leaked, &run, &pushed).unwrap_err(),
    "the blocked push reached the remote"
  );
  let silent = host(root.path(), "echo pushed", false);
  assert!(
    push(&silent, &run, &pushed)
      .unwrap_err()
      .contains("push review")
  );
}
