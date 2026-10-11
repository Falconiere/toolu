use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::{FIXER_AGENT, Run, args, config_content, fixer_env, spawn};
use crate::dispatch::procs::{group_alive, stop_group};

fn fixer(worktree: &Path) -> Run<'_> {
  Run {
    model: Some("probe/m"),
    effort: Some("high"),
    unattended: true,
    worktree,
    prompt: "Read /b.md",
  }
}

fn executable(path: &Path, text: &str) {
  std::fs::write(path, text).unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_config_adds_only_the_fixer_agent_and_keeps_every_caller_key() {
  let added: Value = serde_json::from_str(&config_content(None).unwrap()).unwrap();
  assert_eq!(added.as_object().unwrap().len(), 1);
  assert_eq!(
    added["agent"][FIXER_AGENT]["permission"]["bash"]["git * push *"],
    "deny"
  );
  assert_eq!(added["agent"][FIXER_AGENT]["permission"]["task"], "deny");
  let caller = json!({"model": "probe/default", "permission": {"bash": "ask"}, "agent": {"build": {"model": "probe/build"}}});
  let merged: Value =
    serde_json::from_str(&config_content(Some(&caller.to_string())).unwrap()).unwrap();
  assert_eq!(merged["model"], "probe/default");
  assert_eq!(merged["permission"], caller["permission"]);
  assert_eq!(merged["agent"]["build"], caller["agent"]["build"]);
  assert_eq!(merged["agent"].as_object().unwrap().len(), 2);
  assert!(config_content(Some("  ")).unwrap().contains(FIXER_AGENT));
  for bad in ["{", "[]", "\"text\"", "{\"agent\": []}"] {
    let failure = config_content(Some(bad)).err().unwrap();
    assert!(failure.message.contains("OPENCODE_CONFIG_CONTENT"), "{bad}");
  }
}

#[test]
fn the_argv_names_the_agent_worktree_approvals_model_and_variant() {
  let worktree = Path::new("/w t");
  let expected = [
    "run",
    "--format",
    "json",
    "--dir",
    "/w t",
    "--agent",
    FIXER_AGENT,
    "--auto",
    "--model",
    "probe/m",
    "--variant",
    "high",
    "Read /b.md",
  ];
  assert_eq!(args(&fixer(worktree)).unwrap(), expected);
  let plain = Run {
    model: None,
    effort: None,
    unattended: false,
    ..fixer(worktree)
  };
  assert_eq!(
    args(&plain).unwrap(),
    [
      "run",
      "--format",
      "json",
      "--dir",
      "/w t",
      "--agent",
      FIXER_AGENT,
      "Read /b.md"
    ]
  );
}

#[test]
fn under_the_fixer_env_git_refuses_network_remotes_and_tokens_are_gone() {
  let base = Env::from_pairs([
    ("GH_TOKEN", "t1"),
    ("GITHUB_TOKEN", "t2"),
    ("GH_ENTERPRISE_TOKEN", "t3"),
    ("GITHUB_ENTERPRISE_TOKEN", "t4"),
    ("GIT_ALLOW_PROTOCOL", "file:https"),
    ("KEEP", "yes"),
    ("PATH", "/usr/bin:/bin"),
  ]);
  let env = fixer_env(&base, Path::new("/w"), Path::new("/gh")).unwrap();
  for key in [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
  ] {
    assert_eq!(env.get(key), None);
  }
  assert_eq!(
    (env.get("KEEP"), env.get("PWD"), env.get("GH_CONFIG_DIR")),
    (Some("yes"), Some("/w"), Some("/gh"))
  );
  assert_eq!(
    (
      env.get("GIT_ALLOW_PROTOCOL"),
      env.get("GIT_TERMINAL_PROMPT")
    ),
    (Some("file"), Some("0"))
  );
  let dir = tempfile::tempdir().unwrap();
  for (command, transport) in [
    (
      "git ls-remote https://github.com/Falconiere/toolu.git",
      "https",
    ),
    ("git ls-remote git@github.com:Falconiere/toolu.git", "ssh"),
    ("git ls-remote git://github.com/Falconiere/toolu.git", "git"),
  ] {
    let mut spec = Spec::new(["sh", "-c", command]);
    spec.env = Some(env.clone().with("HOME", &dir.path().display().to_string()));
    spec.cwd = Some(dir.path().to_path_buf());
    let output = run(&spec).unwrap();
    assert_ne!(output.exit_code, 0, "{command}");
    assert!(
      output
        .stderr
        .contains(&format!("transport '{transport}' not allowed")),
      "{}",
      output.stderr
    );
  }
}

#[test]
fn a_missing_or_unexecutable_opencode_is_reported_and_logged() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir(&bin).unwrap();
  let log = dir.path().join("fixer.log");
  let gh = dir.path().join("gh");
  let bare = Env::from_pairs([("PATH", "/usr/bin:/bin")]);
  assert_eq!(
    spawn(&fixer(dir.path()), &log, &bare, &gh),
    Err("opencode is not on PATH".to_owned())
  );
  executable(&bin.join("opencode"), "#!/nonexistent/interpreter\n");
  let env = Env::from_pairs([("PATH", format!("{}:/usr/bin:/bin", bin.display()))]);
  let error = spawn(&fixer(dir.path()), &log, &env, &gh).err().unwrap();
  assert_eq!(
    error,
    format!(
      "opencode did not start; its error goes to {}",
      log.display()
    )
  );
  assert!(
    std::fs::read_to_string(&log)
      .unwrap()
      .starts_with("opencode did not start: ")
  );
}

#[test]
fn a_started_fixer_runs_detached_until_its_group_is_stopped() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir(&bin).unwrap();
  executable(
    &bin.join("opencode"),
    "#!/bin/sh\necho \"{\\\"type\\\":\\\"start\\\",\\\"pwd\\\":\\\"$PWD\\\"}\"\nsleep 30 &\nwait\n",
  );
  let env = Env::from_pairs([
    ("PATH", format!("{}:/usr/bin:/bin", bin.display())),
    ("GH_TOKEN", "secret".to_owned()),
  ]);
  let log = dir.path().join("fixer.log");
  let (pid, start) = spawn(&fixer(dir.path()), &log, &env, &dir.path().join("gh")).unwrap();
  assert!(group_alive(pid, &start));
  let started = Instant::now();
  while !std::fs::read_to_string(&log).unwrap().contains("start")
    && started.elapsed() < Duration::from_secs(5)
  {
    std::thread::sleep(Duration::from_millis(20));
  }
  assert!(
    std::fs::read_to_string(&log)
      .unwrap()
      .contains(&dir.path().display().to_string())
  );
  assert!(stop_group(pid, &start));
  assert!(!group_alive(pid, &start));
}
