//! `setup` adds the notes ref to `notes.rewriteRef` once, and fails outside a
//! repository.

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::setup;
use crate::cli::Place;
use crate::git_support::{Repo, git};

#[test]
fn setup_adds_the_rewrite_ref_once() {
  let repo = Repo::new(".claude", None);
  let place = Place {
    env: repo.env.clone(),
    dir: repo.root.clone(),
  };
  let json = Ctx {
    json: true,
    ..Ctx::default()
  };
  let first: Value = serde_json::from_str(&setup(&json, &place).stdout.unwrap()).unwrap();
  assert_eq!(first["changed"], true);
  let again = setup(&Ctx::default(), &place).stdout.unwrap();
  assert_eq!(
    again,
    "notes.rewriteRef already includes refs/notes/git-better"
  );
  let refs = git(&repo.root, &["config", "--get-all", "notes.rewriteRef"]);
  assert_eq!(refs, "refs/notes/git-better\n");
  let empty = tempfile::tempdir().unwrap();
  let outside = Place {
    env: repo.env,
    dir: empty.path().to_path_buf(),
  };
  assert_eq!(setup(&Ctx::default(), &outside).exit, Exit::Failure);
}
