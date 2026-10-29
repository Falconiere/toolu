use std::path::Path;

use toolu_runtime::env::Env;

use super::{Tmux, TmuxError, WorktreeRow, session_name_for};

/// A tmux client on a fresh socket, with a scratch directory for worktrees.
struct World {
  _dir: tempfile::TempDir,
  client: Tmux,
  repo: std::path::PathBuf,
}

impl World {
  fn new() -> World {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("tmux.sock");
    let env = Env::from_pairs([("PATH", "/usr/bin:/bin:/opt/homebrew/bin")]);
    let client = Tmux::new(&env, Some(socket));
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    World {
      _dir: dir,
      client,
      repo,
    }
  }

  /// A git repository with one committed file on `main`.
  fn git_repo(&self) {
    let repo = &self.repo;
    run_git(repo, &["init", "-q"]);
    run_git(repo, &["config", "user.email", "test@example.com"]);
    run_git(repo, &["config", "user.name", "Test"]);
    std::fs::write(repo.join("file.txt"), "hello\n").unwrap();
    run_git(repo, &["add", "file.txt"]);
    run_git(repo, &["commit", "-q", "-m", "init"]);
  }
}

fn run_git(cwd: &Path, args: &[&str]) {
  let mut command = vec![
    "git".to_owned(),
    "-C".to_owned(),
    cwd.to_string_lossy().into_owned(),
  ];
  command.extend(args.iter().map(|arg| (*arg).to_owned()));
  let mut spec = toolu_runtime::process::Spec::new(command);
  spec.env = Some(Env::from_pairs([(
    "PATH",
    "/usr/bin:/bin:/opt/homebrew/bin",
  )]));
  let output = toolu_runtime::process::run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "git {args:?}: {}", output.stderr);
}

#[test]
fn a_session_is_created_listed_and_killed() {
  let world = World::new();
  let client = &world.client;
  assert!(!client.session_exists("toolu-test"));
  client.create_session("toolu-test", &world.repo).unwrap();
  assert!(client.session_exists("toolu-test"));
  assert!(
    client
      .list_sessions()
      .unwrap()
      .contains(&"toolu-test".to_owned())
  );
  client.kill_session("toolu-test").unwrap();
  assert!(!client.session_exists("toolu-test"));
}

#[test]
fn creating_an_existing_session_fails() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-dup", &world.repo).unwrap();
  let err = client.create_session("toolu-dup", &world.repo).unwrap_err();
  assert_eq!(err.code, "workspace_exists");
}

#[test]
fn literal_text_is_sent_and_captured() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-echo", &world.repo).unwrap();
  let pane = client.first_pane("toolu-echo").unwrap();
  client.send_literal(&pane, "echo hello-world").unwrap();
  // Give the shell a moment to run the command.
  std::thread::sleep(std::time::Duration::from_millis(300));
  let text = client.capture(&pane, 5).unwrap();
  assert!(text.contains("hello-world"), "{text}");
}

#[test]
fn the_first_pane_has_a_pid() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-pid", &world.repo).unwrap();
  let pane = client.first_pane("toolu-pid").unwrap();
  let pid = client.pane_pid(&pane).unwrap();
  assert!(pid > 0);
}

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
fn named_keys_reach_the_pane() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-keys", &world.repo).unwrap();
  let pane = client.first_pane("toolu-keys").unwrap();
  client.send_literal(&pane, "cat").unwrap();
  std::thread::sleep(std::time::Duration::from_millis(200));
  client.send_keys(&pane, &["C-c"]).unwrap();
}

#[test]
fn a_missing_pane_fails_lookups() {
  let world = World::new();
  let client = &world.client;
  // Start the server with an unrelated session so the socket exists.
  client.create_session("toolu-anchor", &world.repo).unwrap();
  let err: TmuxError = client.pane_pid("%999999").unwrap_err();
  assert_eq!(err.code, "pane_not_found");
}

#[test]
fn a_missing_session_fails_the_first_pane_lookup() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-anchor", &world.repo).unwrap();
  let err = client.first_pane("toolu-nope").unwrap_err();
  assert_eq!(err.code, "pane_not_found");
}

#[test]
fn killing_a_missing_session_is_a_no_op() {
  let world = World::new();
  assert!(world.client.kill_session("toolu-ghost").is_ok());
}

#[test]
fn listing_sessions_on_an_empty_server_is_empty_or_erroring() {
  let world = World::new();
  // No session has been created yet, so the server may not even be running.
  let sessions = world.client.list_sessions().unwrap_or_default();
  assert_eq!(sessions, Vec::<String>::new());
}

#[test]
fn a_status_message_is_sent_to_the_session() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-notify", &world.repo).unwrap();
  client
    .display_message("toolu-notify", "hello from the test")
    .unwrap();
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

#[test]
fn an_unknown_tmux_subcommand_fails() {
  let world = World::new();
  let err = world.client.run(&["not-a-real-subcommand"]).unwrap_err();
  assert_eq!(err.code, "tmux");
}
