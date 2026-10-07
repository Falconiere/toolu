//! Push waivers across implementations (#421, AC-6): `push-waiver.ts` and
//! [`Waivers`] resolve the same files for one project, write the same bytes,
//! and each promotes and matches the marker the other recorded.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use toolu_engine::waiver::Waivers;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const MODULE: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../../packages/toolu-core/src/ledger/push-waiver.ts"
);

/// One `push-waiver.ts` call for `feat_x`, printed with the files it resolves.
const SCRIPT: &str = "const w = await import(process.env.WAIVER);
const o = { env: { HOME: process.env.W_HOME }, host: 'claude', now: () => new Date(Number(process.env.W_NOW) * 1000) };
const [root, sha] = [process.env.W_ROOT, process.env.W_SHA];
const result = process.env.W_OP === 'pend' ? w.pushWaiverPend(root, 'feat_x', sha, 'main', 'findings', o)
  : process.env.W_OP === 'promote' ? w.pushWaiverPromote(root, 'feat_x', sha, o)
  : w.pushWaiverMatches(root, 'feat_x', sha, o);
console.log(JSON.stringify({ result, path: w.pushWaiverPath(root, 'feat_x', o), pending: w.pushWaiverPendingPath(root, 'feat_x', o) }));";

/// A project and the home its state lives under.
struct Side {
  _dir: tempfile::TempDir,
  home: String,
  root: String,
}

impl Side {
  fn new() -> Res<Side> {
    let dir = tempfile::tempdir()?;
    let base = std::fs::canonicalize(dir.path())?;
    let (home, root) = (base.join("home"), base.join("project"));
    std::fs::create_dir_all(&home)?;
    let init = Command::new("git")
      .args(["init", "-q"])
      .arg(&root)
      .status()?;
    if !init.success() {
      return Err("git init failed".into());
    }
    let text = |path: &Path| path.display().to_string();
    Ok(Side {
      home: text(&home),
      root: text(&root),
      _dir: dir,
    })
  }

  /// `push-waiver.ts` doing `op` for `sha` at epoch second `now`.
  fn typescript(&self, op: &str, sha: &str, now: u64) -> Res<Value> {
    let output = Command::new("bun")
      .args(["-e", SCRIPT])
      .env("WAIVER", MODULE)
      .env("W_HOME", &self.home)
      .env("W_ROOT", &self.root)
      .env("W_OP", op)
      .env("W_SHA", sha)
      .env("W_NOW", now.to_string())
      .output()?;
    if !output.status.success() {
      return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
  }
}

/// A file's permission bits, or 0 when it cannot be read.
fn mode(path: &str) -> u32 {
  std::fs::metadata(path).map_or(0, |meta| meta.permissions().mode() & 0o777)
}

fn at(seconds: u64) -> SystemTime {
  UNIX_EPOCH + Duration::from_secs(seconds)
}

const ASKED: u64 = 1_927_857_906;
const WAIVED: u64 = 1_927_857_966;

/// Runs `test` with a fresh project and its `feat_x` waivers.
fn with_waivers(test: impl FnOnce(&Side, &Waivers<'_>)) -> Res<()> {
  let side = Side::new()?;
  let env = Env::from_pairs([("HOME", side.home.as_str())]);
  let roots = Roots::new(env, Some(Host::Claude));
  let root = Path::new(&side.root);
  test(
    &side,
    &Waivers {
      roots: &roots,
      root: Some(root),
      slug: "feat_x",
    },
  );
  Ok(())
}

#[test]
fn rust_promotes_a_typescript_marker_written_with_the_same_bytes() {
  with_waivers(|side, waivers| {
    let pended = side.typescript("pend", "A", ASKED).unwrap();
    assert_eq!(pended["result"], true);
    assert_eq!(pended["path"], waivers.path().as_str());
    assert_eq!(pended["pending"], waivers.pending_path().as_str());
    let typescript_marker = std::fs::read(waivers.pending_path()).unwrap();
    assert!(waivers.pend("A", "main", "findings", at(ASKED)));
    assert_eq!(
      std::fs::read(waivers.pending_path()).unwrap(),
      typescript_marker
    );

    side.typescript("pend", "A", ASKED).unwrap();
    assert!(
      !waivers.promote("B", at(WAIVED)),
      "a marker for another sha waives nothing"
    );
    assert!(
      !Path::new(&waivers.path()).exists(),
      "promoting B left a waiver"
    );
    assert!(
      waivers.promote("A", at(WAIVED)),
      "Rust cashes in the TypeScript marker"
    );
    assert_eq!(mode(&waivers.path()), 0o600);
    assert_eq!(side.typescript("matches", "A", 0).unwrap()["result"], true);
    assert_eq!(side.typescript("matches", "B", 0).unwrap()["result"], false);
  })
  .unwrap();
}

#[test]
fn typescript_promotes_a_rust_marker_to_the_waiver_rust_writes() {
  with_waivers(|side, waivers| {
    assert!(waivers.pend("B", "main", "findings", at(ASKED)));
    assert_eq!(
      side.typescript("promote", "B", WAIVED).unwrap()["result"],
      true
    );
    assert!(
      waivers.matches("B"),
      "Rust finds the waiver TypeScript promoted"
    );
    assert!(!waivers.matches("A"));
    assert_eq!(mode(&waivers.path()), 0o600);
    let typescript_waiver = std::fs::read(waivers.path()).unwrap();
    assert!(waivers.pend("B", "main", "findings", at(ASKED)));
    assert!(waivers.promote("B", at(WAIVED)));
    assert_eq!(std::fs::read(waivers.path()).unwrap(), typescript_waiver);
  })
  .unwrap();
}
