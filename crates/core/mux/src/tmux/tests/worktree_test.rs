use std::path::Path;

use crate::test_support::World;

use super::{WorktreeRow, session_name_for};

#[test]
fn a_worktree_is_created_listed_and_removed() {
  let world = World::new();
  world.git_repo();
  let client = &world.client;
  let worktree = client
    .create_worktree(&world.repo, "feature", "main", "feature")
    .unwrap();
  assert!(Path::new(&worktree.path).is_dir());
  assert!(client.session_exists(&worktree.workspace_id));

  let rows: Vec<WorktreeRow> = client.list_worktrees(&world.repo).unwrap();
  let row = rows
    .iter()
    .find(|row| row.branch == "feature")
    .expect("feature worktree listed");
  assert_eq!(row.path, worktree.path);
  assert_eq!(
    row.open_workspace_id.as_deref(),
    Some(worktree.workspace_id.as_str())
  );

  client
    .remove_worktree(&world.repo, &worktree.workspace_id, true)
    .unwrap();
  assert!(!client.session_exists(&worktree.workspace_id));
  assert!(!Path::new(&worktree.path).exists());
}

#[test]
fn an_open_worktree_reuses_its_session() {
  let world = World::new();
  world.git_repo();
  let client = &world.client;
  let worktree = client
    .create_worktree(&world.repo, "feature", "main", "feature")
    .unwrap();
  client.kill_session(&worktree.workspace_id).unwrap();
  let reopened = client
    .open_worktree(Path::new(&worktree.path), "feature")
    .unwrap();
  assert_eq!(reopened.workspace_id, worktree.workspace_id);
  assert!(client.session_exists(&reopened.workspace_id));
}

#[test]
fn session_names_are_safe_for_tmux() {
  assert_eq!(session_name_for("issue-42"), "toolu-issue-42");
  assert_eq!(session_name_for("a.b:c"), "toolu-a-b-c");
}

#[test]
fn removing_an_unknown_workspace_fails() {
  let world = World::new();
  world.git_repo();
  world
    .client
    .create_session("toolu-anchor", &world.repo)
    .unwrap();
  let err = world
    .client
    .remove_worktree(&world.repo, "toolu-missing", true)
    .unwrap_err();
  assert_eq!(err.code, "workspace_not_found");
}

#[test]
fn opening_a_missing_worktree_fails() {
  let world = World::new();
  let err = world
    .client
    .open_worktree(Path::new("/nonexistent/path"), "feature")
    .unwrap_err();
  assert_eq!(err.code, "worktree_not_found");
}

#[test]
fn a_bare_repository_lists_no_worktrees() {
  let world = World::new();
  world.git_repo();
  let rows = world.client.list_worktrees(&world.repo).unwrap();
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].branch, "main");
  assert_eq!(rows[0].open_workspace_id, None);
}

#[test]
fn a_worktree_result_renders_the_herdr_shape() {
  let world = World::new();
  world.git_repo();
  let worktree = world
    .client
    .create_worktree(&world.repo, "feature", "main", "feature")
    .unwrap();
  let value = worktree.to_value();
  assert_eq!(value["worktree"]["path"], worktree.path);
  assert_eq!(value["workspace"]["workspace_id"], worktree.workspace_id);
  assert_eq!(value["root_pane"]["pane_id"], worktree.pane_id);
}
