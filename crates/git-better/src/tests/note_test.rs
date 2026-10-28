//! The provenance note: its JSON, the file cap, and an idempotent write to
//! `refs/notes/git-better` in a real repository.

use serde_json::json;

use super::{MAX_FILES, NOTES_REF, Provenance, body, write};
use crate::areas::AREA_ROOTS;
use crate::git_support::Repo;
use crate::gitio::FileChange;

fn provenance() -> Provenance {
  Provenance {
    branch: "feat/1-x".to_owned(),
    base: "main".to_owned(),
    host: "claude".to_owned(),
    session: "s1".to_owned(),
  }
}

fn roots() -> Vec<String> {
  AREA_ROOTS.iter().map(|root| (*root).to_owned()).collect()
}

#[test]
fn the_body_records_provenance_areas_and_files() {
  let renamed = FileChange {
    path: "crates/x/b.rs".to_owned(),
    old_path: Some("crates/x/a.rs".to_owned()),
    status: "R".to_owned(),
    added: Some(2),
    deleted: Some(1),
  };
  assert_eq!(
    body(&provenance(), &[renamed], &roots()),
    json!({
      "schema": "git-better/v1",
      "branch": "feat/1-x",
      "base": "main",
      "host": "claude",
      "session": "s1",
      "areas": [{ "area": "crates/x", "files": 1, "added": 2, "deleted": 1 }],
      "files": [{ "path": "crates/x/b.rs", "oldPath": "crates/x/a.rs", "status": "R",
                  "added": 2, "deleted": 1 }],
      "truncated": false,
    })
  );
}

#[test]
fn a_large_commit_lists_the_first_files_and_says_so() {
  let many: Vec<FileChange> = (0..=MAX_FILES)
    .map(|index| FileChange {
      path: format!("docs/{index}.md"),
      old_path: None,
      status: "A".to_owned(),
      added: None,
      deleted: None,
    })
    .collect();
  let note = body(&provenance(), &many, &roots());
  assert_eq!(note["files"].as_array().map(Vec::len), Some(MAX_FILES));
  assert_eq!(note["truncated"], true);
  assert_eq!(note["areas"][0]["files"], MAX_FILES + 1);
}

#[test]
fn a_note_is_written_once_per_commit() {
  let repo = Repo::new(".claude", None);
  repo.commit("README.md", "docs: x", None);
  let (env, root) = (&repo.env, repo.root.as_path());
  assert_eq!(write(env, root, "HEAD", &provenance(), &roots()), Ok(true));
  assert_eq!(write(env, root, "HEAD", &provenance(), &roots()), Ok(false));
  let shown = crate::git_support::git(root, &["notes", "--ref", NOTES_REF, "show", "HEAD"]);
  let note: serde_json::Value = serde_json::from_str(&shown).unwrap();
  assert_eq!(
    note["areas"],
    json!([{ "area": ".", "files": 1, "added": 1, "deleted": 0 }])
  );
}
