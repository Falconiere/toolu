use std::path::Path;

use super::{Claude, argv};
use crate::host_smoke_prompts::CLAUDE_COMMIT;
use crate::host_smoke_steps::{Driver, Step};

#[test]
fn the_command_line_loads_both_plugins_and_ends_with_the_prompt() {
  let step = Step {
    name: "deny",
    tools: "Read,Edit",
    prompt: "do it",
  };
  let words = argv(Path::new("/bin/claude"), Path::new("/repo"), &step);
  let line = words.join(" ");
  assert_eq!(words.first().map(String::as_str), Some("/bin/claude"));
  assert!(line.contains("--plugin-dir /repo/plugins/toolu --plugin-dir /repo/plugins/ts-quality"));
  assert!(line.contains("--model haiku -p --permission-mode auto"));
  assert!(line.contains("--output-format stream-json --verbose --tools Read,Edit"));
  assert!(line.ends_with("--max-budget-usd 2 do it"));
}

#[test]
fn the_claude_driver_stages_inside_its_commit_prompt() {
  let claude = Claude {
    exe: "claude".into(),
    repo: "/repo".into(),
  };
  let prompts = claude.prompts();
  assert_eq!(claude.name(), "claude");
  assert!(!prompts.stages_first);
  assert_eq!(prompts.commit, CLAUDE_COMMIT);
  assert!(prompts.commit.contains("git add sample.ts && git commit"));
  assert!(
    prompts
      .push
      .contains("git push origin HEAD:refs/heads/feat/smoke")
  );
}

#[test]
fn the_driver_argv_matches_the_free_function() {
  let claude = Claude {
    exe: "/bin/claude".into(),
    repo: "/repo".into(),
  };
  let step = Step {
    name: "allowed",
    tools: "Bash",
    prompt: "pwd",
  };
  let direct = argv(Path::new("/bin/claude"), Path::new("/repo"), &step);
  assert_eq!(Driver::argv(&claude, &step, Path::new("/project")), direct);
}
