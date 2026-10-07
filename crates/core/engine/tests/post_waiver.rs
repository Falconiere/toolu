//! push-waiver through the post-tool walk on real repositories (the scenarios
//! of `push-waiver.test.ts`): which pushes promote the pending marker, and the
//! repository whose state the `-C` chain picks.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::path::{Path, PathBuf};
use std::process::Command;

use hook::Hook;
use sandbox::{Res, write};
use serde_json::{Value, json};
use toolu_engine::Phase;
use toolu_engine::builtins::POST_TOOL;
use toolu_engine::trace::StepKind;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_state::diff_sha::diff_sha;

const DIR: &str = "project/.claude/tmp/push-review";

fn git(dir: &Path, args: &[&str]) -> Res<()> {
  let ok = Command::new("git")
    .args([
      "-c",
      "user.email=t@t",
      "-c",
      "user.name=t",
      "-c",
      "commit.gpgsign=false",
    ])
    .args(args)
    .current_dir(dir)
    .status()
    .map_err(|err| err.to_string())?
    .success();
  if ok {
    Ok(())
  } else {
    Err(format!("git {args:?} failed"))
  }
}

fn commit(dir: &Path, file: &str) -> Res<()> {
  write(&dir.join(file), &format!("{file}\n"))?;
  git(dir, &["add", "-A"])?;
  git(dir, &["commit", "-q", "-m", file])
}

/// The marker `pushWaiverPend` writes for `root`'s diff against `main`.
fn pend(root: &Path, dir: &Path, slug: &str) -> Res<()> {
  let env = Env::from_pairs([(
    "PATH",
    std::env::var("PATH").map_err(|err| err.to_string())?,
  )]);
  let sha = diff_sha(&env, root, "main").ok_or("no diff sha")?;
  let marker = format!(
    "{{\"version\":1,\"branch\":\"{slug}\",\"diff_sha\":\"{sha}\",\"base_branch\":\"main\",\"reason_code\":\"no-state\",\"asked_at\":\"2026-10-07T00:00:00Z\"}}\n"
  );
  write(&dir.join(format!("{slug}.pending-waiver.json")), &marker)
}

/// `feat/example` one commit ahead of `main`, with `STATE_DIR` and `PUSH_REVIEW_BASE` set.
fn repo(pending: bool) -> Res<Hook> {
  let mut hook = Hook::new(Host::Claude)?;
  let project = hook.sb.path("project");
  git(&project, &["symbolic-ref", "HEAD", "refs/heads/main"])?;
  commit(&project, "base.txt")?;
  git(&project, &["checkout", "-q", "-b", "feat/example"])?;
  commit(&project, "feature.txt")?;
  let dir = hook.sb.path(DIR);
  hook.extra = vec![
    ("STATE_DIR".to_owned(), dir.to_string_lossy().into_owned()),
    ("PUSH_REVIEW_BASE".to_owned(), "main".to_owned()),
  ];
  if pending {
    pend(&project, &dir, "feat_example")?;
  }
  Ok(hook)
}

fn push(command: &str, response: &Value) -> String {
  json!({"tool_name": "Bash", "tool_input": {"command": command}, "tool_response": response})
    .to_string()
}

fn ok(command: &str) -> String {
  push(
    command,
    &json!({"exit_code": 0, "stdout": "", "interrupted": false}),
  )
}

/// Dispatches `stdin`; whether the project's waiver exists afterwards.
fn promoted(hook: &Hook, stdin: &str) -> bool {
  let dispatched = hook.run(Phase::Post, stdin, POST_TOOL, &[]);
  assert_eq!(
    (
      dispatched.result.stdout.as_str(),
      dispatched.result.exit_code
    ),
    ("", 0),
    "{stdin}"
  );
  hook.sb.path(DIR).join("feat_example.waiver.json").exists()
}

#[test]
fn a_landed_push_promotes_and_anything_else_does_not() {
  let cases = [
    (ok("git push"), true),
    (push("git push", &json!({"exit_code": 1})), false),
    (
      push("git push", &json!({"stdout": "", "interrupted": true})),
      false,
    ),
    (
      push("git push", &json!({"stdout": "", "interrupted": false})),
      true,
    ),
    (ok("git status"), false),
    (ok("cat <<'EOF' > notes.txt\ngit push\nEOF"), false),
    (ok("git $SUB"), false),
    (
      json!({"tool_name": "Edit", "tool_input": {"file_path": "a.ts"}, "tool_response": {}})
        .to_string(),
      false,
    ),
  ];
  for (stdin, want) in cases {
    assert_eq!(promoted(&repo(true).unwrap(), &stdin), want, "{stdin}");
  }
}

#[test]
fn wrapped_and_nested_pushes_promote() {
  for command in [
    "timeout 120 git push",
    "/usr/bin/git push",
    "bash -c \"git push\"",
    "eval git push",
  ] {
    assert!(promoted(&repo(true).unwrap(), &ok(command)), "{command}");
  }
  let hook = repo(true).unwrap();
  let project = hook.sb.text("project");
  assert!(promoted(&hook, &ok(&format!("git -C {project} push"))));
}

#[test]
fn the_waiver_names_the_pushed_diff_and_drops_the_marker() {
  let hook = repo(true).unwrap();
  assert!(promoted(&hook, &ok("git push")));
  let dir = hook.sb.path(DIR);
  assert!(!dir.join("feat_example.pending-waiver.json").exists());
  let waiver: Value =
    serde_json::from_str(&std::fs::read_to_string(dir.join("feat_example.waiver.json")).unwrap())
      .unwrap();
  assert_eq!(
    (waiver["version"].as_i64(), waiver["branch"].as_str()),
    (Some(1), Some("feat_example"))
  );
  assert!(waiver.get("asked_at").is_none() && waiver["waived_at"].is_string());
}

#[test]
fn no_marker_or_a_marker_for_older_code_waives_nothing() {
  assert!(!promoted(&repo(false).unwrap(), &ok("git push")));
  let hook = repo(true).unwrap();
  commit(&hook.sb.path("project"), "more.txt").unwrap();
  assert!(!promoted(&hook, &ok("git push")));
}

/// The project on `feat/example` and a worktree `wt` on `feat/wt`, each with a
/// marker for its own diff in its own state dir; no `STATE_DIR` or base override.
fn two_repos() -> Res<(Hook, PathBuf)> {
  let mut hook = repo(false)?;
  hook.extra.clear();
  let project = hook.sb.path("project");
  let wt = hook.sb.path("wt");
  let wt_text = wt.to_string_lossy().into_owned();
  git(
    &project,
    &["worktree", "add", "-q", "-b", "feat/wt", &wt_text, "main"],
  )?;
  commit(&wt, "wt.txt")?;
  pend(
    &project,
    &project.join(".claude/tmp/push-review"),
    "feat_example",
  )?;
  pend(&wt, &wt.join(".claude/tmp/push-review"), "feat_wt")?;
  Ok((hook, wt))
}

fn waivers(hook: &Hook, wt: &Path) -> Vec<String> {
  let project = hook
    .sb
    .path("project/.claude/tmp/push-review/feat_example.waiver.json");
  let worktree = wt.join(".claude/tmp/push-review/feat_wt.waiver.json");
  [("project", project), ("wt", worktree)]
    .into_iter()
    .filter(|(_, path)| path.exists())
    .map(|(name, _)| name.to_owned())
    .collect()
}

#[test]
fn the_push_is_judged_on_the_repository_it_pushes() {
  for (command, want) in [
    ("git -C ../wt push", "wt"),
    ("git -C .. -C wt push", "wt"),
    ("git push", "project"),
    ("git -C \"$WT\" push", "project"),
  ] {
    let (hook, wt) = two_repos().unwrap();
    hook.run(Phase::Post, &ok(command), POST_TOOL, &[]);
    assert_eq!(waivers(&hook, &wt), [want], "{command}");
  }
  let (hook, wt) = two_repos().unwrap();
  hook.run(
    Phase::Post,
    &ok(&format!("git -C {} push", wt.display())),
    POST_TOOL,
    &[],
  );
  assert_eq!(waivers(&hook, &wt), ["wt"], "an absolute -C");
}

#[test]
fn the_built_ins_run_in_table_order_before_the_registry() {
  let hook = repo(true).unwrap();
  hook.sh(Phase::Post, "x@t__after.sh", "exit 0").unwrap();
  assert!(hook.dir(Phase::Post).starts_with(hook.config_root()));
  let dispatched = hook.run(Phase::Post, &ok("git push"), POST_TOOL, &[]);
  let order: Vec<(&str, StepKind)> = dispatched
    .trace
    .iter()
    .map(|step| (step.module.as_str(), step.kind))
    .collect();
  assert_eq!(
    order,
    [
      ("gate-status.sh", StepKind::Builtin),
      ("push-waiver.sh", StepKind::Builtin),
      ("x@t__after.sh", StepKind::Executable),
    ]
  );
}
