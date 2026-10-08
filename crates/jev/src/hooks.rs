//! Test support for the hook modules: a temp home, a temp plugin root whose shim is the one
//! `plugins/jev/scripts/jev.sh` will be, and the checks every hook test makes of an `Outcome`.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Output, Spec, run};

/// The key a hook may or may not receive.
pub(crate) const KEY: &str = "local-test-key";

/// A temp tree: `home`, and `plugin` with the shim and the skill.
pub(crate) struct Sandbox {
  pub(crate) dir: TempDir,
  pub(crate) home: PathBuf,
  pub(crate) plugin: PathBuf,
}

/// A plugin root at `dir` with the shim (mode 0755) and the skill file.
pub(crate) fn install(dir: &Path) -> PathBuf {
  fs::create_dir_all(dir.join("scripts")).unwrap();
  fs::create_dir_all(dir.join("skills/jev")).unwrap();
  let shim = dir.join("scripts/jev.sh");
  fs::write(&shim, "#!/bin/sh\nexec toolu jev \"$@\"\n").unwrap();
  fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
  fs::write(dir.join("skills/jev/SKILL.md"), "# jev\n").unwrap();
  dir.to_path_buf()
}

impl Sandbox {
  pub(crate) fn new() -> Sandbox {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir(&home).unwrap();
    let plugin = install(&dir.path().join("plugin"));
    Sandbox { dir, home, plugin }
  }

  /// `HOME`, the key, and `host` named through `TOOLU_HOST_OVERRIDE`.
  pub(crate) fn env(&self, host: &str) -> Env {
    Env::from_pairs([
      ("HOME", self.home.to_str().unwrap()),
      ("TYPESAFE_API_KEY", KEY),
      ("TOOLU_HOST_OVERRIDE", host),
    ])
  }

  /// `<home>/.claude/jev`, where Claude's shim lands.
  pub(crate) fn claude_dir(&self) -> PathBuf {
    self.home.join(".claude/jev")
  }
}

/// The context of a successful hook run: no stderr, no trailing line feed, the exact
/// `hookSpecificOutput` object and nothing else.
pub(crate) fn context_of(outcome: &Outcome, event: &str) -> String {
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stderr, None);
  let text = outcome.stdout.as_deref().unwrap();
  assert!(text.ends_with('}') && !text.contains('\n'));
  let value = Ordered::parse(text).unwrap();
  assert_eq!(value.to_text(false), text);
  assert!(matches!(&value, Ordered::Object(top) if top.len() == 1));
  let output = value.get("hookSpecificOutput").unwrap();
  assert!(matches!(output, Ordered::Object(keys) if keys.len() == 2));
  assert_eq!(
    output.get("hookEventName"),
    Some(&Ordered::String(event.into()))
  );
  match output.get("additionalContext") {
    Some(Ordered::String(context)) => context.clone(),
    other => panic!("no additionalContext: {other:?}"),
  }
}

/// The outcome of a hook with nothing to say.
pub(crate) fn assert_silent(outcome: &Outcome) {
  assert_eq!(
    (outcome.exit, &outcome.stdout, &outcome.stderr),
    (Exit::Success, &None, &None)
  );
}

/// The command a mandate tells the agent to call.
pub(crate) fn called(context: &str) -> &str {
  let (_, rest) = context.split_once("you MUST call ").unwrap();
  rest.split_once(" before the decision").unwrap().0
}

/// `/bin/sh -c command` with exactly `env` and its output.
pub(crate) fn sh(command: &str, env: &Env) -> Output {
  let mut spec = Spec::new(["/bin/sh", "-c", command]);
  spec.env = Some(env.clone());
  run(&spec).unwrap()
}

/// The skill line every non-`OpenCode` host gets for `plugin`.
pub(crate) fn skill_file(plugin: &Path) -> String {
  format!("{}/skills/jev/SKILL.md", plugin.display())
}

/// The expected mandate for `command` and `skill`.
pub(crate) fn mandate(command: &str, skill: &str) -> String {
  format!(
    "Jev is mandatory on every task containing semantic decisions. After initial exploration, \
identify useful judgments over supplied evidence; you MUST call {command} before the decision it \
informs. The command is toolu jev and does not load a project .env file. Reassess after new \
evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask \
call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no \
semantic decision, say so in one sentence rather than skipping silently. Syntax and linked \
examples: {skill}. Keep exact rules, tests, and code verification deterministic. On service \
failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or \
authorization."
  )
}

/// The first `node` on this process's `PATH`, if any.
pub(crate) fn node() -> Option<PathBuf> {
  let env = Env::process();
  env
    .get("PATH")?
    .split(':')
    .map(|dir| Path::new(dir).join("node"))
    .find(|path| path.is_file())
}

#[cfg(test)]
#[path = "tests/hooks_test.rs"]
mod tests;
