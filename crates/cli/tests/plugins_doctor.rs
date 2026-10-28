//! `toolu plugins doctor` against stub hosts: inventory, secrets env warn, JSON shape.

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct Sb {
  dir: tempfile::TempDir,
}

impl Sb {
  fn new() -> Res<Sb> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-plugins-doctor-")
      .tempdir()?;
    std::fs::create_dir_all(dir.path().join("bin"))?;
    Ok(Sb { dir })
  }

  fn root(&self) -> PathBuf {
    self.dir.path().to_path_buf()
  }

  fn stub(&self, name: &str, listing: &str) -> Res<()> {
    let temp = self.root().join(format!("bin/.{name}"));
    let body = format!(
      "#!/bin/sh\ncase \"$*\" in\n*list*--json*) printf '%s\\n' '{listing}' ;;\nesac\nexit 0\n"
    );
    std::fs::write(&temp, body)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&temp, self.root().join(format!("bin/{name}")))?;
    Ok(())
  }

  fn stub_fail(&self, name: &str, stderr: &str) -> Res<()> {
    let temp = self.root().join(format!("bin/.{name}"));
    let body = format!("#!/bin/sh\nprintf '%s\\n' '{stderr}' >&2\nexit 1\n");
    std::fs::write(&temp, body)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&temp, self.root().join(format!("bin/{name}")))?;
    Ok(())
  }

  fn run(&self, args: &[&str]) -> Res<Output> {
    let path = format!("{}:/usr/bin:/bin", self.root().join("bin").display());
    let mut command = Command::new(TOOLU);
    command
      .args(args)
      .current_dir(self.root())
      .env_clear()
      .env("PATH", path)
      .env("HOME", self.root())
      .stdin(Stdio::null());
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }
}

fn text(out: &Output) -> String {
  String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn doctor_json_reports_namespace_and_warns_on_empty_inventory() {
  let sb = Sb::new().unwrap();
  sb.stub("claude", "[]").unwrap();
  let out = sb
    .run(&["--json", "plugins", "doctor", "--host", "claude"])
    .unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out));
  let doc: Value = serde_json::from_str(&text(&out)).unwrap();
  assert_eq!(doc["namespace"], "plugins");
  assert_eq!(doc["verb"], "doctor");
  assert_eq!(doc["ok"], true);
  assert!(doc["counts"]["warn"].as_u64().unwrap() >= 1);
  let summary = doc["hosts"][0]["findings"]
    .as_array()
    .unwrap()
    .iter()
    .find(|finding| finding["scope"] == "host")
    .and_then(|finding| finding["summary"].as_str())
    .unwrap_or("");
  assert!(summary.contains("no toolu plugin"), "{summary} in {doc}");
}

#[test]
fn doctor_warns_when_code_review_lacks_openrouter_key() {
  let sb = Sb::new().unwrap();
  sb.stub(
    "claude",
    r#"[{"id":"code-review@toolu","version":"8.6.0","scope":"user","enabled":true}]"#,
  )
  .unwrap();
  let out = sb
    .run(&["--json", "plugins", "doctor", "--host", "claude"])
    .unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out));
  let doc: Value = serde_json::from_str(&text(&out)).unwrap();
  let secrets: Vec<_> = doc["hosts"][0]["findings"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|finding| finding["scope"] == "secrets")
    .collect();
  assert!(
    secrets
      .iter()
      .any(|finding| { finding["name"] == "OPENROUTER_API_KEY" && finding["status"] == "warn" }),
    "{doc}"
  );
}

#[test]
fn doctor_fails_when_host_list_fails_without_echoing_stderr() {
  let sb = Sb::new().unwrap();
  sb.stub_fail("claude", "CANARY_SECRET_stderr").unwrap();
  let out = sb
    .run(&["--json", "plugins", "doctor", "--host", "claude"])
    .unwrap();
  assert_eq!(out.status.code(), Some(1), "{}", text(&out));
  let shown = format!("{}{}", text(&out), String::from_utf8_lossy(&out.stderr));
  assert!(
    !shown.contains("CANARY_SECRET_stderr"),
    "stderr leaked: {shown}"
  );
  let doc: Value = serde_json::from_str(&text(&out)).unwrap();
  assert_eq!(doc["ok"], false);
  assert!(
    doc["hosts"][0]["findings"]
      .as_array()
      .unwrap()
      .iter()
      .any(|finding| finding["status"] == "fail" && finding["scope"] == "host"),
    "{doc}"
  );
}
