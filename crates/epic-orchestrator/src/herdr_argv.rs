//! CLI argv for one herdr control call.

use toolu_runtime::env::Env;

use crate::herdr::{Call, session_name};

impl Call {
  /// `herdr` argv, including `--session` when the env names one.
  pub(crate) fn argv(&self, env: &Env) -> Vec<String> {
    let mut argv = prefix(env);
    match self {
      Self::Start { .. } => start_argv(&mut argv, self),
      Self::Prompt { target, text } => {
        argv.extend([
          "agent".into(),
          "prompt".into(),
          target.clone(),
          text.clone(),
        ]);
      }
      Self::Read { target } => argv.extend([
        "agent".into(),
        "read".into(),
        target.clone(),
        "--source".into(),
        "recent-unwrapped".into(),
        "--lines".into(),
        "15".into(),
        "--format".into(),
        "text".into(),
      ]),
      Self::Create { .. } => create_argv(&mut argv, self),
      Self::Remove { .. } => remove_argv(&mut argv, self),
    }
    argv
  }
}

fn prefix(env: &Env) -> Vec<String> {
  let mut argv = vec!["herdr".to_owned()];
  if let Some(name) = session_name(env) {
    argv.push("--session".to_owned());
    argv.push(name.to_owned());
  }
  argv
}

fn start_argv(argv: &mut Vec<String>, call: &Call) {
  let Call::Start {
    name,
    kind,
    pane_id,
    args: extra,
    timeout_ms,
  } = call
  else {
    return;
  };
  argv.extend([
    "agent".into(),
    "start".into(),
    name.clone(),
    "--kind".into(),
    kind.clone(),
  ]);
  argv.push("--pane".into());
  argv.push(pane_id.clone());
  if let Some(timeout) = timeout_ms {
    argv.push("--timeout".into());
    argv.push(timeout.to_string());
  }
  if !extra.is_empty() {
    argv.push("--".into());
    argv.extend(extra.iter().cloned());
  }
}

fn create_argv(argv: &mut Vec<String>, call: &Call) {
  let Call::Create {
    cwd,
    branch,
    base,
    path,
    label,
    workspace,
  } = call
  else {
    return;
  };
  argv.extend(["worktree".into(), "create".into()]);
  push_flag(argv, "--cwd", cwd.as_deref());
  push_flag(argv, "--branch", branch.as_deref());
  push_flag(argv, "--base", base.as_deref());
  push_flag(argv, "--path", path.as_deref());
  push_flag(argv, "--label", label.as_deref());
  push_flag(argv, "--workspace", workspace.as_deref());
}

fn remove_argv(argv: &mut Vec<String>, call: &Call) {
  let Call::Remove {
    workspace_id,
    force,
  } = call
  else {
    return;
  };
  argv.extend([
    "worktree".into(),
    "remove".into(),
    "--workspace".into(),
    workspace_id.clone(),
  ]);
  if *force {
    argv.push("--force".into());
  }
}

fn push_flag(argv: &mut Vec<String>, flag: &str, value: Option<&str>) {
  if let Some(value) = value {
    argv.push(flag.to_owned());
    argv.push(value.to_owned());
  }
}

/// The five control calls the socket and CLI tests share.
pub(crate) fn sample_calls() -> [Call; 5] {
  [
    Call::Start {
      name: "a".into(),
      kind: "claude".into(),
      pane_id: "w1:p1".into(),
      args: vec!["status".into()],
      timeout_ms: Some(1000),
    },
    Call::Prompt {
      target: "a".into(),
      text: "STATUS?".into(),
    },
    Call::Read { target: "a".into() },
    Call::Create {
      cwd: Some("/work".into()),
      branch: Some("feat".into()),
      base: Some("main".into()),
      path: Some("/work".into()),
      label: Some("a".into()),
      workspace: Some("w1".into()),
    },
    Call::Remove {
      workspace_id: "w1".into(),
      force: true,
    },
  ]
}

#[cfg(test)]
#[path = "tests/herdr_argv_test.rs"]
mod tests;
