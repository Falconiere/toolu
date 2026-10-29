//! `toolu completions` against the real binary: each shell's script, and the
//! usage errors for a missing shell, an unknown shell, and `--json`.

use std::error::Error;
use std::process::Output;

use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn Error>>;

fn toolu(args: &[&str]) -> Res<Output> {
  let resources = tempfile::tempdir()?;
  Ok(
    assert_cmd::Command::new(env!("CARGO_BIN_EXE_toolu"))
      .args(args)
      .env("TOOLU_RESOURCE_HOME", resources.path())
      .output()?,
  )
}

fn stdout(output: &Output) -> Res<String> {
  Ok(String::from_utf8(output.stdout.clone())?)
}

fn stderr(output: &Output) -> Res<String> {
  Ok(String::from_utf8(output.stderr.clone())?)
}

fn script(shell: &str) -> Res<String> {
  let output = toolu(&["completions", shell])?;
  if output.status.code() != Some(0) {
    return Err(format!("{shell} exited {:?}", output.status.code()).into());
  }
  let diagnostic = stderr(&output)?;
  if !diagnostic.is_empty() {
    return Err(format!("{shell} wrote a diagnostic: {diagnostic}").into());
  }
  stdout(&output)
}

/// Whether the fish script offers `name` as a subcommand of `parent`.
fn fish_offers(script: &str, parent: &str, name: &str) -> bool {
  let seen = format!("__fish_toolu_using_subcommand {parent}");
  let choice = format!("-a \"{name}\"");
  script
    .lines()
    .any(|line| line.contains(&seen) && line.contains(&choice))
}

#[test]
fn bash_mentions_a_visible_command_and_a_host_value() {
  let script = script("bash").unwrap();
  assert!(script.contains("plugins"), "{script}");
  assert!(script.contains("claude"), "{script}");
}

#[test]
fn zsh_emits_a_toolu_compdef() {
  let script = script("zsh").unwrap();
  assert!(
    script.contains("#compdef toolu") || script.contains("compdef _toolu toolu"),
    "{script}"
  );
}

#[test]
fn fish_completes_toolu_and_omits_a_hidden_hook_verb() {
  let script = script("fish").unwrap();
  assert!(script.contains("complete -c toolu"), "{script}");
  assert!(fish_offers(&script, "jev", "noul"), "{script}");
  assert!(!fish_offers(&script, "jev", "hook"), "{script}");
}

#[test]
fn a_missing_or_unknown_shell_exits_64() {
  for args in [&["completions"][..], &["completions", "powershell"]] {
    let output = toolu(args).unwrap();
    assert_eq!(output.status.code(), Some(64), "{args:?}");
    assert_eq!(stdout(&output).unwrap(), "", "{args:?}");
    assert!(!stderr(&output).unwrap().is_empty(), "{args:?}");
  }
}

#[test]
fn json_exits_64_with_one_error_document() {
  let output = toolu(&["completions", "bash", "--json"]).unwrap();
  assert_eq!(output.status.code(), Some(64));
  let document: Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(
    document,
    json!({
      "error": {
        "code": 64,
        "name": "usage",
        "message": "toolu completions prints a shell script, not a JSON document",
        "suggestion": null
      }
    })
  );
}
