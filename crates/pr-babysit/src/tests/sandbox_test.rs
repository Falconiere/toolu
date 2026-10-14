//! Test worlds for the dispatcher: a repository with a bare origin holding
//! `main` and the PR branch `feat/fix`, the captured slot state, a
//! plan, and a `bin` directory first on `PATH` for host CLIs.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::dispatcher::Options;
use super::start::Start;
use crate::fixture;

/// A fixer's work: the fix for `sum.ts`, committed.
pub(crate) const FIX: &str = "printf 'export const sum = (a: number, b: number) => a + b;\\n' > sum.ts\n\
                              git commit --quiet -am 'fix(sum): add its arguments'";

/// One sandbox.
pub(crate) struct Sandbox {
  pub(crate) dir: TempDir,
  pub(crate) env: Env,
  pub(crate) repo: PathBuf,
  pub(crate) origin: PathBuf,
  pub(crate) state: PathBuf,
  pub(crate) toolu: PathBuf,
  pub(crate) root: String,
}

/// Runs git in `cwd` with `env`; its trimmed stdout, panicking on failure.
pub(crate) fn git(env: &Env, cwd: &Path, args: &[&str]) -> String {
  let mut command = vec!["git"];
  command.extend_from_slice(args);
  let mut spec = Spec::new(command);
  spec.env = Some(env.clone());
  spec.cwd = Some(cwd.to_path_buf());
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "git {args:?}: {}", output.stderr);
  output.stdout.trim().to_owned()
}

/// A git identity and no user or system git config, with `bin` first on `PATH`.
fn identity(root: &Path) -> Env {
  std::fs::write(root.join("gitconfig"), "").unwrap();
  std::fs::create_dir(root.join("bin")).unwrap();
  let bin = root.join("bin").display().to_string();
  Env::from_pairs([
    ("HOME", root.display().to_string()),
    ("PATH", format!("{bin}:/usr/bin:/bin")),
    ("GIT_AUTHOR_NAME", "Babysit Test".into()),
    ("GIT_AUTHOR_EMAIL", "babysit@example.test".into()),
    ("GIT_COMMITTER_NAME", "Babysit Test".into()),
    ("GIT_COMMITTER_EMAIL", "babysit@example.test".into()),
    ("GIT_CONFIG_NOSYSTEM", "1".into()),
    (
      "GIT_CONFIG_GLOBAL",
      root.join("gitconfig").display().to_string(),
    ),
  ])
}

/// `repo` on `feat/fix`, both branches pushed to the bare `origin`.
fn repository(env: &Env, root: &Path, repo: &Path, origin: &Path) {
  let (repo_text, origin_text) = (repo.display().to_string(), origin.display().to_string());
  git(
    env,
    root,
    &["init", "--quiet", "--bare", "-b", "main", &origin_text],
  );
  git(env, root, &["init", "--quiet", "-b", "main", &repo_text]);
  std::fs::write(
    repo.join("sum.ts"),
    "export const sum = (a: number, b: number) => a - b;\n",
  )
  .unwrap();
  for step in [
    &["add", "sum.ts"][..],
    &["commit", "--quiet", "-m", "init"],
    &["remote", "add", "origin", &origin_text],
    &["push", "--quiet", "origin", "main"],
    &["checkout", "--quiet", "-b", "feat/fix"],
  ] {
    git(env, repo, step);
  }
  std::fs::write(repo.join("sum.test.ts"), "// failing test\n").unwrap();
  for step in [
    &["add", "sum.test.ts"][..],
    &["commit", "--quiet", "-m", "feat: add test"],
    &["push", "--quiet", "origin", "feat/fix"],
  ] {
    git(env, repo, step);
  }
}

/// A sandbox with the repository and the slot state at `.toolu/tmp/pr-babysit`.
pub(crate) fn sandbox() -> Sandbox {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().to_path_buf();
  let env = identity(&root);
  let (repo, origin) = (root.join("repo"), root.join("origin.git"));
  repository(&env, &root, &repo, &origin);
  let mut state = fixture::json("states/toolu-165-initial.json");
  state["actions"]["replied"] =
    json!({"2300000001": {"at": "2026-10-03T12:00:00Z", "replyId": 7234}});
  let path = repo.join(".toolu/tmp/pr-babysit/falconiere-toolu-165.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, state.to_string()).unwrap();
  let root_text = repo.display().to_string();
  Sandbox {
    dir,
    env,
    repo,
    origin,
    state: path,
    toolu: root.join("bin/toolu"),
    root: root_text,
  }
}

impl Sandbox {
  /// Writes `bin/<name>` with `script`, executable.
  pub(crate) fn bin(&self, name: &str, script: &str) {
    let path = self.dir.path().join("bin").join(name);
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
  }

  /// A herdr stand-in and a `claude` CLI: `worktree create` adds a git
  /// worktree, `agent prompt` runs `work` in it and reports `done`, and
  /// `agent start` answers `start`.
  pub(crate) fn herdr(&self, start: &str, work: &str) {
    let worktree = self.dir.path().join("herdr-wt").display().to_string();
    let script = format!(
      r#"#!/bin/sh
case "$1 $2" in
"worktree create")
  git -C "$4" worktree add --quiet -b "$6" '{worktree}' "$8" >/dev/null 2>&1
  echo '{{"result":{{"worktree":{{"path":"{worktree}"}},"workspace":{{"workspace_id":"w1"}},"root_pane":{{"pane_id":"p1"}}}}}}';;
"worktree remove") git -C '{root}' worktree remove --force '{worktree}' && echo '{{"result":{{}}}}';;
"pane list") echo '{{"result":{{"panes":[{{"pane_id":"p1"}}]}}}}';;
"agent start") echo '{start}';;
"agent prompt")
  brief=${{4#You are a pr-babysit fixer. Read }}
  brief=${{brief% and follow it exactly.}}
  cd '{worktree}' && {work}
  printf '{{"version":1,"status":"done","note":"fixed","at":"x"}}' > "${{brief%.md}}.report.json"
  echo '{{"result":{{}}}}';;
"agent wait") echo '{{"result":{{"agent":{{"agent_status":"idle"}}}}}}';;
"agent read") echo 'Fixed sum.ts';;
"agent list") echo '{{"result":{{"agents":[]}}}}';;
*) echo '{{"result":{{}}}}';;
esac
"#,
      root = self.root,
    );
    self.bin("herdr", &script);
    self.bin("claude", "#!/bin/sh\n");
  }

  /// Writes `plan.json` and `items.json`: one item routed to `host`.
  pub(crate) fn plan(&self, host: &str, model: Option<&str>) -> (PathBuf, PathBuf) {
    let items = self.dir.path().join("items.json");
    let item = json!({"id": "ci:test", "kind": "ci", "path": "sum.ts", "task": "Make sum add its arguments."});
    std::fs::write(&items, json!({"round": 1, "items": [item]}).to_string()).unwrap();
    let plan = self.dir.path().join("plan.json");
    let group = json!({"seq": 1, "tier": "trivial", "host": host, "model": model, "effort": null, "items": ["ci:test"]});
    let document =
      json!({"version": 1, "dispatch": "herdr", "unattended": true, "groups": [group]});
    std::fs::write(&plan, document.to_string()).unwrap();
    (plan, items)
  }

  /// The dispatcher options over this sandbox's state.
  pub(crate) fn options(&self, dry: bool) -> Options<'_> {
    Options {
      state_file: &self.state,
      env: &self.env,
      toolu: &self.toolu,
      dry,
    }
  }

  /// `start`'s inputs for `plan` and `items` on `feat/fix` over `main` in the repository.
  pub(crate) fn start<'a>(&'a self, plan: &'a Path, items: &'a Path) -> Start<'a> {
    Start {
      plan,
      items,
      repo_root: &self.root,
      branch: "feat/fix",
      base: "main",
    }
  }

  /// The slot state as written.
  pub(crate) fn state(&self) -> Value {
    serde_json::from_str(&std::fs::read_to_string(&self.state).unwrap()).unwrap()
  }
}

#[test]
fn the_sandbox_holds_a_pushed_pr_branch_and_the_captured_slot_state() {
  let sb = sandbox();
  let origin = git(&sb.env, &sb.origin, &["rev-parse", "feat/fix"]);
  assert_eq!(origin, git(&sb.env, &sb.repo, &["rev-parse", "HEAD"]));
  assert_eq!(sb.state()["slot"], "falconiere-toolu-165");
  assert_eq!(
    sb.state()["actions"]["replied"]["2300000001"]["replyId"],
    7234
  );
  let (plan, items) = sb.plan("codex", None);
  assert!(plan.exists() && items.exists());
  assert_eq!(sb.options(true).state_file, sb.state);
  let start = sb.start(&plan, &items);
  assert_eq!((start.branch, start.base), ("feat/fix", "main"));
  assert_eq!(start.repo_root, sb.repo.display().to_string());
}
