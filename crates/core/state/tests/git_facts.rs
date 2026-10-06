//! Git facts against git itself (AC-5, AC-6): on repositories built by real
//! `git` (a normal repo, a linked worktree, a submodule, detached and unborn
//! HEADs, packed refs, `--separate-git-dir`, a `.git` file to a bare
//! repository, a bare repository, inside `.git`, no repository, a branch that
//! is also a tag), `toplevel`, `current_branch`, `linked_worktree` and
//! `common_dir` give what `git rev-parse` prints; with `PATH` empty they give
//! the same, so nothing was spawned.

use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::{common_dir, current_branch, linked_worktree, toplevel};

type Res<T> = Result<T, String>;

/// A scratch directory with an isolated HOME for git.
struct Scratch {
  _dir: tempfile::TempDir,
  root: PathBuf,
  home: PathBuf,
}

impl Scratch {
  fn new() -> Res<Scratch> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
    let home = root.join("home");
    mkdir(&home)?;
    Ok(Scratch {
      _dir: dir,
      root,
      home,
    })
  }

  /// `git <args>` in `cwd`: whether it succeeded, and its stdout.
  fn git(&self, cwd: &Path, args: &[&str]) -> Res<(bool, String)> {
    let out = Command::new("git")
      .args([
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "-c",
        "init.defaultBranch=main",
      ])
      .args([
        "-c",
        "protocol.file.allow=always",
        "-c",
        "commit.gpgsign=false",
      ])
      .args(args)
      .current_dir(cwd)
      .env_clear()
      .env(
        "PATH",
        std::env::var("PATH").map_err(|err| err.to_string())?,
      )
      .env("HOME", &self.home)
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .output()
      .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout)
      .trim_end_matches('\n')
      .to_owned();
    Ok((out.status.success(), stdout))
  }

  fn must(&self, cwd: &Path, args: &[&str]) -> Res<()> {
    match self.git(cwd, args)? {
      (true, _) => Ok(()),
      (false, _) => Err(format!("git {args:?} failed in {}", cwd.display())),
    }
  }

  /// A repository named `name` with one commit, on `branch`.
  fn repo(&self, name: &str, branch: &str) -> Res<PathBuf> {
    let dir = self.root.join(name);
    mkdir(&dir)?;
    self.must(&dir, &["init", "-q", "-b", branch])?;
    self.must(&dir, &["commit", "-q", "--allow-empty", "-m", "c"])?;
    Ok(dir)
  }
}

fn mkdir(dir: &Path) -> Res<()> {
  std::fs::create_dir_all(dir).map_err(|err| err.to_string())
}

#[derive(Debug, PartialEq, Eq)]
struct Facts {
  toplevel: Option<PathBuf>,
  branch: String,
  linked: bool,
  common: Option<PathBuf>,
}

fn ours(env: &Env, cwd: &Path) -> Facts {
  Facts {
    toplevel: toplevel(env, cwd),
    branch: current_branch(env, cwd),
    linked: linked_worktree(env, cwd),
    common: common_dir(env, cwd),
  }
}

fn gits(scratch: &Scratch, cwd: &Path) -> Res<Facts> {
  let (ok, top) = scratch.git(cwd, &["rev-parse", "--show-toplevel"])?;
  let (_, branch) = scratch.git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
  let dirs = [
    "rev-parse",
    "--path-format=absolute",
    "--git-dir",
    "--git-common-dir",
  ];
  let (dirs_ok, dirs) = scratch.git(cwd, &dirs)?;
  let mut lines = dirs.lines().map(|line| line.trim_end_matches('/'));
  let (git_dir, common) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
  Ok(Facts {
    toplevel: ok.then(|| PathBuf::from(top)),
    branch,
    linked: dirs_ok && git_dir != common,
    common: (dirs_ok && !common.is_empty()).then(|| PathBuf::from(common)),
  })
}

type Layout = (&'static str, PathBuf, bool);

/// A repository on `feat/x` reached directly and through a symlink, its
/// linked worktree and its submodule.
fn worktree_layouts(s: &Scratch) -> Res<Vec<Layout>> {
  let n = s.repo("n", "feat/x")?;
  mkdir(&n.join("a/b"))?;
  std::os::unix::fs::symlink(n.join("a/b"), s.root.join("link")).map_err(|e| e.to_string())?;
  s.must(&n, &["worktree", "add", "-q", "../wt", "-b", "feat/wt"])?;
  let sub_src = s.repo("sub-src", "main")?;
  s.must(
    &n,
    &[
      "submodule",
      "add",
      "-q",
      &sub_src.display().to_string(),
      "sub",
    ],
  )?;
  s.must(&n, &["commit", "-q", "-m", "sub"])?;
  Ok(vec![
    ("normal root", n.clone(), true),
    ("normal through a symlink", s.root.join("link"), true),
    ("linked worktree", s.root.join("wt"), true),
    ("submodule", n.join("sub"), true),
  ])
}

/// HEAD and ref storage variants.
fn head_layouts(s: &Scratch) -> Res<Vec<Layout>> {
  let detached = s.repo("detached", "main")?;
  s.must(&detached, &["checkout", "-q", "--detach"])?;
  let unborn = s.root.join("unborn");
  mkdir(&unborn)?;
  s.must(&unborn, &["init", "-q"])?;
  let packed = s.repo("packed", "feat/packed")?;
  s.must(&packed, &["pack-refs", "--all"])?;
  Ok(vec![
    ("detached", detached, true),
    ("unborn", unborn, true),
    ("packed refs", packed, true),
  ])
}

/// Git dirs away from the worktree, bare ones, and no repository at all.
fn gitdir_layouts(s: &Scratch) -> Res<Vec<Layout>> {
  let separate = s.root.join("separate");
  mkdir(&separate)?;
  s.must(
    &separate,
    &["init", "-q", "--separate-git-dir", "../separate.git"],
  )?;
  let bare = s.root.join("bare.git");
  mkdir(&bare)?;
  s.must(&bare, &["init", "-q", "--bare"])?;
  let to_bare = s.root.join("to-bare");
  mkdir(&to_bare)?;
  std::fs::write(to_bare.join(".git"), "gitdir: ../bare.git\n").map_err(|e| e.to_string())?;
  let outside = s.root.join("outside");
  mkdir(&outside)?;
  Ok(vec![
    ("separate git dir", separate, true),
    ("gitfile to a bare repository", to_bare, true),
    ("bare repository", bare, true),
    ("inside .git", s.root.join("n/.git/objects"), true),
    ("no repository", outside, true),
  ])
}

/// A branch whose short name a tag shares: git lengthens it, so it is git's.
fn tagged_layout(s: &Scratch) -> Res<Layout> {
  let tagged = s.repo("tagged", "v1")?;
  s.must(&tagged, &["tag", "v1"])?;
  Ok(("branch that is also a tag", tagged, false))
}

fn layouts(s: &Scratch) -> Res<Vec<Layout>> {
  let mut all = worktree_layouts(s)?;
  all.extend(head_layouts(s)?);
  all.extend(gitdir_layouts(s)?);
  all.push(tagged_layout(s)?);
  Ok(all)
}

fn with_git() -> Env {
  let path = std::env::var("PATH").unwrap_or_default();
  Env::from_pairs([("PATH", path.as_str()), ("HOME", "/nonexistent")])
}

fn without_git() -> Env {
  Env::from_pairs([("PATH", ""), ("HOME", "/nonexistent")])
}

#[test]
fn the_layouts_are_what_git_says_they_are() {
  let s = Scratch::new().unwrap();
  let cases = layouts(&s).unwrap();
  assert_eq!(cases.len(), 13);
  let seen: Vec<Facts> = cases
    .iter()
    .map(|(_, cwd, _)| gits(&s, cwd).unwrap())
    .collect();
  assert_eq!(
    seen[1].toplevel,
    Some(s.root.join("n")),
    "through the symlink"
  );
  assert_eq!(
    (seen[1].branch.as_str(), seen[2].branch.as_str()),
    ("feat/x", "feat/wt")
  );
  assert!(seen[2].linked && !seen[3].linked);
  assert_eq!(seen[3].toplevel, Some(s.root.join("n/sub")));
  assert_eq!(seen[3].common, Some(s.root.join("n/.git/modules/sub")));
  assert_eq!(
    (seen[4].branch.as_str(), seen[5].branch.as_str()),
    ("HEAD", "HEAD")
  );
  assert_eq!(seen[6].branch, "feat/packed");
  assert_eq!(seen[7].common, Some(s.root.join("separate.git")));
  assert_eq!(
    [&seen[8].toplevel, &seen[9].toplevel, &seen[10].toplevel],
    [&None; 3]
  );
  assert_eq!(seen[10].branch, "feat/x", "inside .git");
  let nothing = Facts {
    toplevel: None,
    branch: String::new(),
    linked: false,
    common: None,
  };
  assert_eq!(seen[11], nothing);
  assert_eq!(seen[12].branch, "heads/v1");
}

#[test]
fn every_layout_matches_git_and_needs_no_process() {
  let s = Scratch::new().unwrap();
  let cases = layouts(&s).unwrap();
  for (label, cwd, file_only) in &cases {
    let expected = gits(&s, cwd).unwrap();
    assert_eq!(ours(&with_git(), cwd), expected, "{label}");
    if *file_only {
      assert_eq!(ours(&without_git(), cwd), expected, "{label}, PATH empty");
    }
  }
  assert_eq!(
    current_branch(&without_git(), &cases[12].1),
    "",
    "deferred, git unreachable"
  );
}

#[test]
fn the_project_root_spawns_nothing_on_any_host() {
  let s = Scratch::new().unwrap();
  let cases = worktree_layouts(&s).unwrap();
  for host in [Host::Codex, Host::Opencode, Host::Claude] {
    let roots = Roots::new(without_git(), Some(host));
    for (label, cwd, _) in &cases {
      let expected = gits(&s, cwd).unwrap().toplevel;
      assert_eq!(roots.project_root(Some(cwd)), expected, "{label} {host:?}");
    }
  }
}
