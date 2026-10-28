//! `log`, `context` and `pr` over a real feature branch, as text and JSON.

use clap::ArgMatches;
use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};

use super::{context, log, pr};
use crate::cli::{Place, command};
use crate::git_support::{Repo, git};

fn branch() -> (Repo, Place) {
  let repo = Repo::new(".claude", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  repo.commit(
    "crates/x/a.rs",
    "feat(x): add a\n\nIntent: a is needed\nRisk: low",
    None,
  );
  repo.commit(
    "docs/b.md",
    "docs(x): describe a\n\nIntent: readers find a",
    None,
  );
  let place = Place {
    env: repo.env.clone(),
    dir: repo.root.clone(),
  };
  (repo, place)
}

fn verb(args: &[&str]) -> ArgMatches {
  let all = command().try_get_matches_from(args).unwrap();
  all.subcommand().unwrap().1.clone()
}

fn json(outcome: &Outcome) -> Value {
  serde_json::from_str(outcome.stdout.as_deref().unwrap()).unwrap()
}

#[test]
fn log_reads_the_branch_and_takes_a_range_and_a_limit() {
  let (_repo, place) = branch();
  let text = log(&verb(&["git-better", "log"]), &Ctx::default(), &place);
  let stdout = text.stdout.unwrap();
  assert!(
    stdout.contains("docs(x): describe a\n  why: readers find a"),
    "{stdout}"
  );
  let limited = verb(&["git-better", "log", "main..HEAD", "--limit", "1"]);
  let document = json(&log(
    &limited,
    &Ctx {
      json: true,
      ..Ctx::default()
    },
    &place,
  ));
  assert_eq!(document["range"], "main..HEAD");
  assert_eq!(document["commits"].as_array().map(Vec::len), Some(1));
  let empty = log(
    &verb(&["git-better", "log", "HEAD..HEAD"]),
    &Ctx::default(),
    &place,
  );
  assert_eq!(empty.stdout.as_deref(), Some("no commits in HEAD..HEAD"));
}

#[test]
fn context_and_pr_summarize_the_branch() {
  let (_repo, place) = branch();
  let text = context(&verb(&["git-better", "context"]), &Ctx::default(), &place);
  let stdout = text.stdout.unwrap();
  assert!(
    stdout.starts_with("Branch feat/1-x (feat, ref 1: x): 2 commits ahead of main"),
    "{stdout}"
  );
  let as_json = Ctx {
    json: true,
    ..Ctx::default()
  };
  let document = json(&context(
    &verb(&["git-better", "context"]),
    &as_json,
    &place,
  ));
  assert_eq!(document["risk"], "low");
  let draft = pr(&verb(&["git-better", "pr"]), &Ctx::default(), &place)
    .stdout
    .unwrap();
  assert!(
    draft.starts_with("feat(x): x\n\n## Summary\n\n- a is needed"),
    "{draft}"
  );
  let squash = pr(
    &verb(&["git-better", "pr", "--squash"]),
    &Ctx::default(),
    &place,
  );
  assert!(
    squash
      .stdout
      .unwrap()
      .contains("Intent: a is needed; readers find a")
  );
  assert_eq!(
    json(&pr(&verb(&["git-better", "pr"]), &as_json, &place))["verb"],
    "pr"
  );
}

#[test]
fn outside_a_repository_the_reads_fail() {
  let (repo, _place) = branch();
  let empty = tempfile::tempdir().unwrap();
  let outside = Place {
    env: repo.env,
    dir: empty.path().to_path_buf(),
  };
  let failed = context(&verb(&["git-better", "context"]), &Ctx::default(), &outside);
  assert_eq!(failed.exit, Exit::Failure);
}

#[test]
fn pr_on_the_base_branch_has_nothing_to_draft() {
  let (repo, _place) = branch();
  git(&repo.root, &["switch", "-q", "main"]);
  let place = Place {
    env: repo.env.clone(),
    dir: repo.root,
  };
  let refused = pr(&verb(&["git-better", "pr"]), &Ctx::default(), &place);
  assert_eq!(refused.exit, Exit::Failure);
  assert_eq!(
    refused.stderr.as_deref(),
    Some("main is the base branch: there are no branch commits to draft a PR from")
  );
}
