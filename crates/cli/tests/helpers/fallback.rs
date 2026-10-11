//! The transition fallback to the shipped Bun bundle (AC-9).

use crate::sandbox::{Res, STARTUP, Sandbox, on_path};

fn bundle(sandbox: &Sandbox, name: &str, body: &str) -> Res<()> {
  let dist = sandbox.root.join("hooks/dist");
  std::fs::create_dir_all(&dist)?;
  std::fs::write(dist.join(format!("{name}.js")), body)?;
  Ok(())
}

fn bun() -> Res<String> {
  Ok(on_path("bun")?.to_string_lossy().into_owned())
}

#[test]
fn without_toolu_the_bundle_runs_with_an_advisory_and_its_status() {
  let sandbox = Sandbox::new().unwrap();
  let body = "console.log(JSON.stringify({ systemMessage: 'bundle' })); process.exit(3);";
  bundle(&sandbox, "session-start", body).unwrap();
  let run = sandbox
    .launch(
      "SessionStart",
      "session-start",
      STARTUP,
      &[("TOOLU_BUN", &bun().unwrap())],
    )
    .unwrap();
  assert_eq!(run.code, 3, "{run:?}");
  assert_eq!(run.stdout, "{\"systemMessage\":\"bundle\"}\n");
  assert!(
    run
      .stderr
      .starts_with("toolu plugin: native toolu not found, running the Bun bundle")
  );
}

#[test]
fn an_enforcing_bundle_deny_passes_through() {
  let sandbox = Sandbox::new().unwrap();
  bundle(
    &sandbox,
    "pre-tools",
    "process.stdin.resume(); process.exit(2);",
  )
  .unwrap();
  let run = sandbox
    .launch(
      "PreToolUse",
      "pre-tools",
      "{}",
      &[("TOOLU_BUN", &bun().unwrap())],
    )
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(run.stderr.contains("running the Bun bundle"));
}

#[test]
fn a_set_toolu_bin_never_falls_back() {
  let sandbox = Sandbox::new().unwrap();
  bundle(&sandbox, "pre-tools", "process.exit(0);").unwrap();
  let bun = bun().unwrap();
  let extra = [
    ("TOOLU_BUN", bun.as_str()),
    ("TOOLU_BIN", "/nonexistent/toolu"),
  ];
  let run = sandbox
    .launch("PreToolUse", "pre-tools", "{}", &extra)
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(!run.stderr.contains("Bun bundle"));
}

#[test]
fn toolu_runtime_bun_uses_the_shipped_bundle_even_when_native_is_installed() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  bundle(
    &sandbox,
    "session-start",
    "console.log(JSON.stringify({ systemMessage: 'forced Bun fallback' }));",
  )
  .unwrap();
  let bun = bun().unwrap();
  let run = sandbox
    .launch(
      "SessionStart",
      "session-start",
      STARTUP,
      &[("TOOLU_RUNTIME", "bun"), ("TOOLU_BUN", &bun)],
    )
    .unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert_eq!(run.stdout, "{\"systemMessage\":\"forced Bun fallback\"}\n");
  assert!(run.stderr.contains("TOOLU_RUNTIME=bun selected"), "{run:?}");
}

#[test]
fn forced_bun_without_a_bundle_does_not_run_the_installed_native_hook() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  let bun = bun().unwrap();
  let run = sandbox
    .launch(
      "PreToolUse",
      "pre-tools",
      "{}",
      &[("TOOLU_RUNTIME", "bun"), ("TOOLU_BUN", &bun)],
    )
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(run.stderr.contains("toolu is not installed"));
}
