//! `toolu plugins` on Muse (#549): the real binary with a stub `muse` on `PATH`
//! that logs its argv and replays the output real Muse 1.4.11 printed
//! (`fixtures/plugins/muse.json`), and a toolu checkout seeded from a local
//! repository so no test reaches GitHub.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct Sb {
  dir: tempfile::TempDir,
}

impl Sb {
  fn new() -> Res<Sb> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-plugins-muse-")
      .tempdir()?;
    std::fs::create_dir_all(dir.path().join("bin"))?;
    std::fs::create_dir_all(dir.path().join("home"))?;
    Ok(Sb { dir })
  }

  fn root(&self) -> PathBuf {
    self.dir.path().to_path_buf()
  }

  fn home(&self) -> PathBuf {
    self.root().join("home")
  }

  /// An executable `bin/<name>` running `body`, renamed into place.
  fn stub(&self, name: &str, body: &str) -> Res<()> {
    let temp = self.root().join(format!("bin/.{name}"));
    std::fs::write(&temp, format!("#!/bin/sh\n{body}\n"))?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&temp, self.root().join(format!("bin/{name}")))?;
    Ok(())
  }

  /// A `muse` that logs argv and prints `muse-<key>.json` for its listings.
  fn muse(&self) -> Res<()> {
    let root = self.root();
    let root = root.display();
    self.stub(
      "muse",
      &format!(
        "echo \"$*\" >> {root}/muse.log\n\
         case \"$*\" in\n\
         \"plugins list --json\") cat {root}/muse-installed.json 2>/dev/null || echo '{{\"plugins\":[]}}' ;;\n\
         \"plugins list --available --json\") cat {root}/muse-available.json ;;\n\
         \"plugins marketplace list --json\") echo '{{\"marketplaces\":[]}}' ;;\n\
         \"plugins inspect \"*) cat {root}/muse-inspect.json ;;\n\
         *) exit 0 ;;\n\
         esac"
      ),
    )
  }

  /// Writes the captured case `case` at `rel`, its `/home/user` moved here.
  fn capture(&self, case: &str, rel: &str) -> Res<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/plugins/muse.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let found = fixture
      .get("cases")
      .and_then(Value::as_array)
      .and_then(|cases| {
        cases
          .iter()
          .find(|entry| entry.get("name") == Some(&Value::from(case)))
      })
      .and_then(|entry| entry.get("stdout"))
      .ok_or("no such case")?;
    let text = found
      .to_string()
      .replace("/home/user", &self.home().display().to_string());
    std::fs::write(self.root().join(rel), &text)?;
    Ok(text)
  }

  /// The argv lines a stub logged; none when it never ran. Any other read
  /// error fails the test rather than reading as an empty log.
  fn log(&self, name: &str) -> Res<Vec<String>> {
    match std::fs::read_to_string(self.root().join(format!("{name}.log"))) {
      Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
      Err(err) => Err(err.into()),
    }
  }

  fn git(&self, args: &[&str]) -> Res<()> {
    let out = Command::new("git")
      .args(["-c", "user.name=t", "-c", "user.email=t@t"])
      .args(args)
      .env("HOME", self.home())
      .output()?;
    if !out.status.success() {
      return Err(text(&out.stderr).into());
    }
    Ok(())
  }

  /// The toolu checkout at this binary's tag, as a clone of a local repository
  /// whose origin URL is the real one, so the binary finds the tag and fetches
  /// nothing.
  fn seed_checkout(&self) -> Res<String> {
    let seed = self.root().join("seed").display().to_string();
    let dir = self
      .home()
      .join(".local/share/toolu/muse-marketplace")
      .display()
      .to_string();
    let catalog = self.root().join("seed/.agents/plugins");
    std::fs::create_dir_all(&catalog)?;
    std::fs::write(catalog.join("marketplace.json"), "{}")?;
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    self.git(&["init", "--quiet", &seed])?;
    self.git(&["-C", &seed, "add", "--all"])?;
    self.git(&["-C", &seed, "commit", "--quiet", "-m", "seed"])?;
    self.git(&["-C", &seed, "tag", &tag])?;
    self.git(&["clone", "--quiet", &seed, &dir])?;
    self.git(&[
      "-C",
      &dir,
      "remote",
      "set-url",
      "origin",
      "https://github.com/Falconiere/toolu.git",
    ])?;
    Ok(dir)
  }

  fn run(&self, args: &[&str]) -> Res<Output> {
    let path = format!("{}:/usr/bin:/bin", self.root().join("bin").display());
    let mut command = Command::new(TOOLU);
    command
      .args(args)
      .current_dir(self.root())
      .env_clear()
      .env("PATH", path)
      .env("HOME", self.home());
    // Coverage runs keep their profile path, so no `.profraw` lands in the cwd.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }
}

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn muse_alone_on_path_lists_the_captured_plugins_without_asking() {
  let sb = Sb::new().unwrap();
  sb.muse().unwrap();
  let listing = sb.capture("list", "muse-installed.json").unwrap();
  std::fs::write(
    sb.root().join("muse-installed.json"),
    listing.replace("hooky", "brainstorm"),
  )
  .unwrap();
  let out = sb.run(&["plugins", "list"]).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  let stdout = text(&out.stdout);
  assert!(stdout.contains("  = toolu            8.0.4\n"), "{stdout}");
  assert!(
    stdout.contains("  = brainstorm       1.0.1 (disabled)\n"),
    "{stdout}"
  );
  assert!(
    stdout.contains("  - jev              not installed\n"),
    "{stdout}"
  );
  let json = sb
    .run(&["--json", "plugins", "list", "--host", "muse"])
    .unwrap();
  let rows: Value = serde_json::from_slice(&json.stdout).unwrap();
  let toolu = rows
    .as_array()
    .and_then(|rows| rows.iter().find(|row| row["name"] == "toolu"))
    .unwrap();
  assert_eq!(
    (&toolu["installed"], &toolu["version"], &toolu["enabled"]),
    (
      &Value::Bool(true),
      &Value::from("8.0.4"),
      &Value::Bool(true)
    )
  );
  assert_eq!(
    sb.log("muse").unwrap(),
    ["plugins list --json", "plugins list --json"]
  );
}

#[test]
fn with_three_hosts_muse_is_chosen_by_flag_and_never_guessed() {
  let sb = Sb::new().unwrap();
  sb.muse().unwrap();
  for host in ["claude", "codex"] {
    let log = sb.root().join(format!("{host}.log"));
    sb.stub(host, &format!("echo \"$*\" >> {}", log.display()))
      .unwrap();
  }
  let refused = sb.run(&["--json", "plugins", "list"]).unwrap();
  assert_eq!(refused.status.code(), Some(64));
  assert!(
    text(&refused.stdout).contains("several hosts found (claude, codex, muse)"),
    "{}",
    text(&refused.stdout)
  );
  let chosen = sb
    .run(&["--json", "plugins", "list", "--host", "muse"])
    .unwrap();
  assert_eq!(chosen.status.code(), Some(0), "{}", text(&chosen.stderr));
  assert_eq!(sb.log("muse").unwrap(), ["plugins list --json"]);
  assert_eq!(sb.log("claude").unwrap(), Vec::<String>::new());
  assert_eq!(sb.log("codex").unwrap(), Vec::<String>::new());
}

#[test]
fn a_malformed_muse_listing_fails_instead_of_listing_nothing() {
  let sb = Sb::new().unwrap();
  sb.muse().unwrap();
  std::fs::write(sb.root().join("muse-installed.json"), r#"{"plugins":{}}"#).unwrap();
  let out = sb.run(&["plugins", "list", "--host", "muse"]).unwrap();
  assert_eq!(out.status.code(), Some(1));
  assert_eq!(text(&out.stdout), "");
  assert!(
    text(&out.stderr).contains("muse plugins list --json: expected an object"),
    "{}",
    text(&out.stderr)
  );
}

#[test]
fn a_scope_with_muse_is_a_usage_error_naming_it() {
  let sb = Sb::new().unwrap();
  sb.muse().unwrap();
  let out = sb
    .run(&[
      "plugins", "install", "toolu", "--host", "muse", "--scope", "user",
    ])
    .unwrap();
  assert_eq!(out.status.code(), Some(64));
  assert!(
    text(&out.stderr).contains("--scope is for Claude Code; muse has no scope concept"),
    "{}",
    text(&out.stderr)
  );
  assert_eq!(sb.log("muse").unwrap(), Vec::<String>::new());
}

#[test]
fn install_registers_the_checkout_and_approves_only_under_the_flag() {
  let sb = Sb::new().unwrap();
  sb.muse().unwrap();
  let dir = sb.seed_checkout().unwrap();
  sb.capture("available", "muse-available.json").unwrap();
  sb.capture("inspect-review-needed", "muse-inspect.json")
    .unwrap();
  let report = sb
    .run(&["plugins", "install", "toolu", "--host", "muse"])
    .unwrap();
  assert_eq!(report.status.code(), Some(0), "{}", text(&report.stderr));
  assert_eq!(
    text(&report.stdout),
    "  + toolu            installed; awaiting approval: muse plugins approve toolu\n"
  );
  let approved = sb
    .run(&["plugins", "install", "toolu", "--host", "muse", "--approve"])
    .unwrap();
  assert_eq!(
    approved.status.code(),
    Some(0),
    "{}",
    text(&approved.stderr)
  );
  assert_eq!(
    text(&approved.stdout),
    "  + toolu            installed; approved\n"
  );
  let log = sb.log("muse").unwrap();
  assert_eq!(
    log.get(..6),
    Some(
      &[
        "plugins marketplace list --json".to_owned(),
        format!("plugins marketplace add toolu {dir} --json"),
        "plugins list --json".to_owned(),
        "plugins list --available --json".to_owned(),
        "plugins install toolu@toolu --json".to_owned(),
        "plugins inspect toolu --json".to_owned(),
      ][..]
    ),
    "{log:?}"
  );
  let approvals: Vec<&str> = log
    .iter()
    .map(String::as_str)
    .filter(|line| line.contains("approve"))
    .collect();
  assert_eq!(approvals, ["plugins approve toolu --json"]);
}
