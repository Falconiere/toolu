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

/// The deploy skill's frontmatter, parsed as YAML: Claude Code drops a
/// frontmatter it cannot parse, and the hook with it.
fn frontmatter() -> serde_json::Value {
  let skill = std::fs::read_to_string(SKILL).unwrap();
  let (_, rest) = skill.split_once("---\n").unwrap();
  let (yaml, _) = rest.split_once("\n---\n").unwrap();
  crate::ci_yaml::yaml_to_json(yaml).unwrap()
}

#[test]
fn skill_frontmatter_declares_the_blocking_guard_hook() {
  let matter = frontmatter();
  assert_eq!(matter["disable-model-invocation"], true);
  let entries = matter["hooks"]["PreToolUse"].as_array().unwrap();
  assert_eq!(entries.len(), 1, "{entries:?}");
  assert_eq!(entries[0]["matcher"], "Bash");
  let hooks = entries[0]["hooks"].as_array().unwrap();
  assert_eq!(hooks.len(), 1, "{hooks:?}");
  assert_eq!(hooks[0]["type"], "command");
  assert_eq!(hooks[0]["onFailure"], "block");
  let command = hooks[0]["command"].as_str().unwrap();
  assert!(command.contains("cargo xtask deploy-guard"), "{command}");
  assert!(command.contains("$HOME/.cargo/bin"), "{command}");
}

#[test]
fn skill_step_3_runs_deploy_gate_without_a_waiver() {
  let skill = std::fs::read_to_string(SKILL).unwrap();
  let (_, after) = skill.split_once("3. **Gate.**").unwrap();
  let (step, _) = after.split_once("\n4. **").unwrap();
  assert!(step.contains("cargo xtask deploy-gate"), "{step}");
  for waiver in ["confirms", "host-environment failure"] {
    assert!(
      !skill.contains(waiver),
      "the skill still offers a waiver: {waiver}"
    );
  }
}
