use std::path::Path;

use super::{RECEIPT, decide};
use crate::tests::git;

const SKILL: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../.claude/skills/deploy/SKILL.md"
);

/// The deploy skill's `bash` block that follows the `n. **Title.**` heading.
fn skill_block(heading: &str) -> String {
  let skill = std::fs::read_to_string(SKILL).unwrap();
  let (_, after) = skill.split_once(heading).unwrap();
  let (_, block) = after.split_once("```bash\n").unwrap();
  block.split_once("```").unwrap().0.to_owned()
}

/// A repository with one tracked file per commit, and its `HEAD`.
fn repo(commits: usize) -> (tempfile::TempDir, String) {
  let dir = tempfile::tempdir().unwrap();
  git(dir.path(), &["init", "-q"]);
  std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
  for n in 0..commits {
    std::fs::write(dir.path().join("file"), n.to_string()).unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", &format!("commit {n}")]);
  }
  git(
    dir.path(),
    &["update-ref", "refs/remotes/origin/main", "HEAD"],
  );
  let head = git(dir.path(), &["rev-parse", "HEAD"]);
  (dir, head)
}

fn receipt(dir: &Path, text: &str) {
  let path = dir.join(RECEIPT);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

fn payload(command: &str) -> String {
  serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": command } }).to_string()
}

#[test]
fn step_8_publish_line_without_a_receipt_is_blocked_naming_the_receipt() {
  let (dir, _) = repo(1);
  let reason = decide(dir.path(), &payload(&skill_block("8. **Publish.**"))).unwrap_err();
  assert!(reason.contains("deploy-guard: blocked"), "{reason}");
  assert!(
    reason.contains(&format!("no gate receipt at {RECEIPT}")),
    "{reason}"
  );
}

#[test]
fn step_8_publish_line_with_a_receipt_for_a_clean_head_is_allowed() {
  let (dir, head) = repo(1);
  receipt(dir.path(), &format!("{{\"commit\":\"{head}\"}}"));
  assert_eq!(
    decide(dir.path(), &payload(&skill_block("8. **Publish.**"))),
    Ok(())
  );
}

#[test]
fn tag_tap_and_release_writes_without_a_receipt_are_blocked() {
  let (dir, _) = repo(1);
  for command in [
    skill_block("9. **Update the tap"),
    "git -C \"$W/tap\" push".to_owned(),
    "env -u GH_TOKEN gh release upload \"$TAG\" \"$D/SHA256SUMS\"".to_owned(),
    "gh release edit \"$TAG\" --draft=false".to_owned(),
    "gh pr edit 639 --repo Falconiere/toolu --add-label \"autorelease: tagged\"".to_owned(),
    "git push origin v8.4.2".to_owned(),
    "git push --tags".to_owned(),
    "git push origin \"$TAG\"".to_owned(),
    "git push origin 'unterminated".to_owned(),
    "gh \"$(printf release)\" create v1".to_owned(),
    "gh $SUB create v1 # release".to_owned(),
    "git push origin --mirror".to_owned(),
    "git push origin tags/v8.4.2".to_owned(),
    "git push origin tag v8.4.2".to_owned(),
    "git -c push.followTags=true push".to_owned(),
    "cd \"$W/tap\" && git push".to_owned(),
    "(cd tap; git push)".to_owned(),
    "git --git-dir=tap/.git push".to_owned(),
    "GIT_DIR=tap/.git git push".to_owned(),
    "gh release delete-asset v1 foo".to_owned(),
    "gh issue edit 639 --add-label \"autorelease: tagged\"".to_owned(),
    "gh pr edit 639 --add-label \"$L\"".to_owned(),
  ] {
    assert!(
      decide(dir.path(), &payload(&command)).is_err(),
      "allowed: {command}"
    );
  }
}

#[test]
fn ordinary_commands_and_branch_pushes_pass_without_a_receipt() {
  let (dir, _) = repo(1);
  for command in [
    "git status --porcelain",
    "cargo build --release --locked --bin toolu --target aarch64-apple-darwin",
    "gh release view --json tagName --jq .tagName",
    "git push -u origin deploy-gates",
    "\"$W/bin/toolu\" --version",
    "curl -fsSL https://github.com/Falconiere/toolu/releases/latest/download/install.sh | cmp - install.sh",
  ] {
    assert_eq!(decide(dir.path(), &payload(command)), Ok(()), "{command}");
  }
}

#[test]
fn a_receipt_for_an_older_commit_is_blocked_naming_both_shas() {
  let (dir, head) = repo(2);
  let older = git(dir.path(), &["rev-parse", "HEAD~1"]);
  receipt(dir.path(), &format!("{{\"commit\":\"{older}\"}}"));
  let reason = decide(dir.path(), &payload("git push origin \"refs/tags/$TAG\"")).unwrap_err();
  assert!(
    reason.contains(&older) && reason.contains(&head),
    "{reason}"
  );
}

#[test]
fn a_dirty_tracked_file_blocks_a_matching_receipt() {
  let (dir, head) = repo(1);
  receipt(dir.path(), &format!("{{\"commit\":\"{head}\"}}"));
  std::fs::write(dir.path().join("file"), "edited").unwrap();
  let reason = decide(dir.path(), &payload("git push origin \"refs/tags/$TAG\"")).unwrap_err();
  assert!(reason.contains("uncommitted changes"), "{reason}");
}

#[test]
fn a_malformed_receipt_is_blocked() {
  let (dir, _) = repo(1);
  for text in ["{\"commit\":\"nope\"}", "not json", "{}"] {
    receipt(dir.path(), text);
    let reason = decide(dir.path(), &payload("gh release create v1.0.0")).unwrap_err();
    assert!(reason.contains("is malformed"), "{text}: {reason}");
  }
}

#[test]
fn a_payload_that_is_not_json_or_has_no_command_is_refused() {
  let (dir, _) = repo(1);
  for text in ["not json", "", "{\"tool_input\":{}}"] {
    assert!(decide(dir.path(), text).is_err(), "accepted: {text:?}");
  }
}

/// The skill's command prelude with a real tag, as the agent runs it.
fn prelude() -> String {
  skill_block("Start every command with this prelude").replace("<tag>", "v8.4.2")
}

#[test]
fn step_2_preflight_behind_the_prelude_passes_without_a_receipt() {
  let (dir, _) = repo(1);
  for check in [
    "python3 .github/scripts/release_native.py verify-tag \"$TAG\"",
    "git ls-remote --exit-code --tags origin \"refs/tags/$TAG\"",
  ] {
    let command = format!("{}{check}", prelude());
    assert_eq!(decide(dir.path(), &payload(&command)), Ok(()), "{command}");
  }
}

#[test]
fn a_block_names_the_publishing_command_not_the_prelude() {
  let (dir, _) = repo(1);
  let command = format!("{}{}", prelude(), skill_block("8. **Publish.**"));
  let reason = decide(dir.path(), &payload(&command)).unwrap_err();
  assert!(
    reason.contains("blocked `git push origin \"refs/tags/$TAG\"`"),
    "{reason}"
  );
}

#[test]
fn writing_the_receipt_is_blocked_even_beside_a_valid_one() {
  let (dir, head) = repo(1);
  receipt(dir.path(), &format!("{{\"commit\":\"{head}\"}}"));
  let path = dir.path().join(RECEIPT);
  for tool in ["Write", "Edit", "MultiEdit"] {
    let write = serde_json::json!({ "tool_name": tool, "tool_input": { "file_path": path } });
    let reason = decide(dir.path(), &write.to_string()).unwrap_err();
    assert!(
      reason.contains("only cargo xtask deploy-gate writes"),
      "{tool}: {reason}"
    );
  }
  let bash = format!("echo '{{\"commit\":\"{head}\"}}' > target/deploy/gate.json");
  assert!(decide(dir.path(), &payload(&bash)).is_err());
  let other = serde_json::json!({ "tool_name": "Edit", "tool_input": { "file_path": dir.path().join("file") } });
  assert_eq!(decide(dir.path(), &other.to_string()), Ok(()));
}

#[test]
fn a_gated_head_that_is_not_origin_main_is_blocked() {
  let (dir, head) = repo(2);
  git(
    dir.path(),
    &["update-ref", "refs/remotes/origin/main", "HEAD~1"],
  );
  receipt(dir.path(), &format!("{{\"commit\":\"{head}\"}}"));
  let reason = decide(dir.path(), &payload("git push origin \"refs/tags/$TAG\"")).unwrap_err();
  assert!(reason.contains("is not origin/main"), "{reason}");
}

#[test]
fn tagging_a_commit_other_than_head_is_blocked_even_with_a_receipt() {
  let (dir, head) = repo(1);
  receipt(dir.path(), &format!("{{\"commit\":\"{head}\"}}"));
  for command in [
    "git tag -a v8.4.2 HEAD~3 -m x",
    "git tag -a \"$TAG\" -m \"toolu $TAG\" \"$C\"",
  ] {
    let reason = decide(dir.path(), &payload(command)).unwrap_err();
    assert!(reason.contains("must point at HEAD"), "{command}: {reason}");
  }
  for command in [
    "git tag -a \"$TAG\" -m \"toolu $TAG\"",
    "git tag -d v0.0.1",
    "git tag -l 'v8*'",
    "git tag --contains HEAD~1 'v8*'",
  ] {
    assert_eq!(decide(dir.path(), &payload(command)), Ok(()), "{command}");
  }
}

#[test]
fn pushing_a_tag_or_target_on_another_commit_is_blocked_even_with_a_receipt() {
  let (dir, head) = repo(2);
  git(
    dir.path(),
    &["-c", "tag.gpgSign=false", "tag", "v8.4.1", "HEAD~1"],
  );
  receipt(dir.path(), &format!("{{\"commit\":\"{head}\"}}"));
  for command in [
    "git push origin v8.4.1",
    "git push origin HEAD~1:refs/tags/v8.4.2",
    "gh release create v8.4.2 --target HEAD~1",
  ] {
    let reason = decide(dir.path(), &payload(command)).unwrap_err();
    assert!(reason.contains("must point at HEAD"), "{command}: {reason}");
  }
  git(dir.path(), &["-c", "tag.gpgSign=false", "tag", "v8.4.2"]);
  for command in [
    "git push origin v8.4.2",
    "git push origin HEAD:refs/tags/v8.4.3",
    "git push origin refs/tags/v8.4.9",
  ] {
    assert_eq!(decide(dir.path(), &payload(command)), Ok(()), "{command}");
  }
}

#[test]
fn a_configured_push_follow_tags_makes_a_branch_push_publishing() {
  let (dir, _) = repo(1);
  git(dir.path(), &["config", "push.followTags", "true"]);
  assert!(decide(dir.path(), &payload("git push -u origin deploy-gates")).is_err());
}

#[test]
fn file_tools_cannot_write_the_receipt_or_the_snapshot_under_any_spelling() {
  let (dir, _) = repo(1);
  let root = dir.path().display();
  for path in [
    format!("{root}/target/deploy/GATE.JSON"),
    format!("{root}/target/deploy/../deploy/gate.json"),
    format!("{root}/target/./deploy/gate.json"),
    format!("{root}/.claude/tmp/deploy-guard"),
  ] {
    let write = serde_json::json!({ "tool_name": "Write", "tool_input": { "file_path": path } });
    assert!(
      decide(dir.path(), &write.to_string()).is_err(),
      "allowed: {path}"
    );
  }
}

#[test]
fn reading_or_naming_the_receipt_is_not_a_write() {
  let (dir, _) = repo(1);
  for command in [
    "cat target/deploy/gate.json",
    "rg gate.json crates",
    "git commit -m \"guard gate.json writes\"",
  ] {
    assert_eq!(decide(dir.path(), &payload(command)), Ok(()), "{command}");
  }
}
