//! Push targets on real repositories (AC-12): the `-C` chain, a linked worktree
//! in a directory with a space, detached and unborn HEADs, and the fallbacks.

#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::path::Path;
use std::process::Command;

use sandbox::{Res, Sandbox};
use toolu_engine::detect::{push_target_branch, push_target_root};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_shell::analyze;

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

/// A repository on `feat/x` with one commit, and a detached worktree `wt with space`.
fn repo(sb: &Sandbox) -> Res<()> {
  let project = sb.path("project");
  std::fs::create_dir_all(&project).map_err(|err| err.to_string())?;
  git(&project, &["init", "-q", "-b", "feat/x"])?;
  git(&project, &["commit", "-q", "--allow-empty", "-m", "first"])?;
  git(
    &project,
    &[
      "worktree",
      "add",
      "-q",
      "--detach",
      &sb.text("wt with space"),
    ],
  )
}

fn roots(sb: &Sandbox) -> Roots {
  Roots::new(
    Env::from_pairs([("HOME", sb.text("home"))]),
    Some(Host::Codex),
  )
}

#[test]
fn the_c_chain_names_the_pushed_repository_and_dynamic_steps_fall_back() {
  let sb = Sandbox::new().unwrap();
  repo(&sb).unwrap();
  let (project, wt) = (sb.path("project"), sb.path("wt with space"));
  let at = |command: &str| push_target_root(&analyze(command), &roots(&sb), &project);
  assert_eq!(
    at(&format!(
      "git -C \"{}\" push origin HEAD:feat/y",
      wt.display()
    )),
    wt
  );
  assert_eq!(
    at("git -C ../ -C \"wt with space\" push"),
    wt,
    "each -C is relative to the last"
  );
  assert_eq!(at("git -C \"$D\" push"), project);
  assert_eq!(at("git -C missing push"), project);
  sandbox::write(&project.join("sub/file"), "x").unwrap();
  assert_eq!(
    at("git -C sub push"),
    project,
    "a -C into a subdirectory names its toplevel"
  );
  let outside = sb.path("outside");
  sandbox::write(&outside.join("note"), "not a repository").unwrap();
  assert_eq!(
    push_target_root(&analyze("git push"), &roots(&sb), &outside),
    outside
  );
}

#[test]
fn outside_any_repository_the_host_project_root_comes_before_the_cwd() {
  let sb = Sandbox::new().unwrap();
  let outside = sb.path("outside");
  sandbox::write(&outside.join("note"), "not a repository").unwrap();
  let env = Env::from_pairs([
    ("HOME", sb.text("home")),
    ("CLAUDE_PROJECT_DIR", sb.text("declared")),
  ]);
  let claude = Roots::new(env, Some(Host::Claude));
  assert_eq!(
    push_target_root(&analyze("git push"), &claude, &outside),
    sb.path("declared")
  );
}

#[test]
fn the_branch_is_checked_out_or_named_by_the_refspec() {
  let sb = Sandbox::new().unwrap();
  repo(&sb).unwrap();
  let env = Env::from_pairs([("HOME", sb.text("home"))]);
  let (project, wt) = (sb.path("project"), sb.path("wt with space"));
  let branch = |command: &str, root: &Path| push_target_branch(&analyze(command), root, &env);
  assert_eq!(branch("git push origin HEAD:feat/y", &project), "feat/x");
  assert_eq!(branch("git push origin HEAD:feat/y", &wt), "feat/y");
  assert_eq!(branch("git push origin +HEAD:refs/heads/z", &wt), "z");
  for refspec in [":gone", "HEAD", "'refs/*'", "\"$R\"", ""] {
    assert_eq!(
      branch(&format!("git push origin {refspec}"), &wt),
      "",
      "{refspec}"
    );
  }
  let unborn = sb.path("unborn");
  std::fs::create_dir_all(&unborn).unwrap();
  git(&unborn, &["init", "-q", "-b", "main"]).unwrap();
  assert_eq!(branch("git push origin HEAD:topic", &unborn), "topic");
}
