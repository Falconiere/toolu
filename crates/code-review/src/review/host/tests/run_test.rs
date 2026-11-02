use super::super::{
  Argv, Delivery, HostPlan, MAX_ARGV_PROMPT_BYTES, argv, available, binary, delivery, find_on_path,
};
use super::{HostError, run_review};
use toolu_protocol::host::Host;

fn invocation(
  bin: &str,
  host: Host,
  model: &str,
  prompt: &str,
  prompt_file: Option<&str>,
) -> Result<Vec<String>, String> {
  argv(&Argv {
    bin,
    host,
    model,
    schema_file: "/tmp/schema.json",
    prompt,
    prompt_file,
  })
}

#[test]
fn every_host_either_has_a_binary_or_is_hermes() {
  for host in Host::ALL {
    assert_eq!(binary(host).is_some(), host != Host::Hermes);
  }
}

#[test]
fn prompts_never_travel_in_argv_for_stdin_hosts() {
  let big = "x".repeat(MAX_ARGV_PROMPT_BYTES + 1);
  for host in [Host::Claude, Host::Codex, Host::Muse] {
    let prompt_file = if delivery(host) == Delivery::PromptFile {
      Some("/tmp/prompt.txt")
    } else {
      None
    };
    let args = invocation("bin", host, "default", &big, prompt_file).unwrap();
    assert!(!args.iter().any(|arg| arg == &big), "{host:?}");
  }
}

#[test]
fn argv_hosts_refuse_oversized_prompts() {
  let big = "x".repeat(MAX_ARGV_PROMPT_BYTES + 1);
  for host in [Host::Cursor, Host::Opencode] {
    assert!(invocation("bin", host, "default", &big, None).is_err());
  }
}

#[test]
fn argv_hosts_accept_small_prompts_with_model_flag() {
  let args = invocation("/bin/cursor-agent", Host::Cursor, "some-model", "hi", None).unwrap();
  assert_eq!(args.first().map(String::as_str), Some("/bin/cursor-agent"));
  assert!(args.contains(&"--model".to_owned()));
  assert!(args.contains(&"some-model".to_owned()));
  let modelled = invocation("claude", Host::Claude, "default", "hi", None).unwrap();
  assert!(!modelled.iter().any(|arg| arg == "--model"));
  assert!(invocation("bin", Host::Hermes, "default", "hi", None).is_err());
}

#[test]
fn availability_needs_an_executable_binary() {
  assert_eq!(available(None), Vec::new());
  assert_eq!(available(Some("")), Vec::new());
}

#[test]
fn find_on_path_ignores_missing_and_non_executable() {
  assert_eq!(find_on_path("no-such-binary-xyz", "/usr/bin:/bin"), None);
}

fn fake_cli(name: &str, script: &str) -> (tempfile::TempDir, std::path::PathBuf) {
  use std::os::unix::fs::PermissionsExt as _;
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join(name);
  std::fs::write(&bin, script).unwrap();
  std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
  (dir, bin)
}

#[test]
fn a_fake_cli_reviews_end_to_end() {
  let envelope = r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"findings\": []}","usage":{"input_tokens":3,"output_tokens":5},"total_cost_usd":0.01}"#;
  let (_dir, bin) = fake_cli("claude", &format!("#!/bin/sh\nprintf '%s' '{envelope}'"));
  let plan = HostPlan {
    host: Host::Claude,
    model: "default".into(),
    bin,
  };
  let (done, _stdout) = run_review(&plan, "{}", "hi", std::time::Duration::from_secs(10)).unwrap();
  assert_eq!(done.content, "{\"findings\": []}");
  assert_eq!(done.provider.as_deref(), Some("claude"));
  assert_eq!(done.usage.prompt_tokens, 3);
  assert_eq!(done.usage.cost_usd, Some(0.01));
}

#[test]
fn a_failing_cli_is_a_transport_failure() {
  let (_dir, bin) = fake_cli("codex", "#!/bin/sh\nexit 3");
  let plan = HostPlan {
    host: Host::Codex,
    model: "default".into(),
    bin,
  };
  let err = run_review(&plan, "{}", "hi", std::time::Duration::from_secs(10)).unwrap_err();
  assert!(matches!(err, HostError::Failed(_)), "{err:?}");
}
