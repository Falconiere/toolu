//! The Codex prune (AC-11): modules of plugins a ready snapshot does not list are
//! removed from both event directories; symlinks, un-namespaced files, other
//! hosts and untrustworthy snapshots are left alone.

#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::os::unix::fs::symlink;
use std::path::PathBuf;

use sandbox::{Res, Sandbox, write};
use toolu_engine::registry::prune::prune_inactive_modules;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

/// No path removed.
const NONE: [PathBuf; 0] = [];

const FILES: [&str; 8] = [
  "pre-tools.d/a@t__keep.js",
  "pre-tools.d/b@t__drop.js",
  "pre-tools.d/b@t__drop.sh",
  "pre-tools.d/noname.sh",
  "post-tools.d/b@t__rule.json",
  "post-tools.d/a@t__rule.json",
  "post-tools.d/b@t__notes.txt",
  "post-tools.d/.b@t__hidden.sh",
];

/// A Codex home with every file of `FILES`, a symlinked `b@t` module and `snapshot`.
fn codex_home(sb: &Sandbox, snapshot: Option<&str>) -> Res<Roots> {
  for file in FILES {
    write(&sb.path(&format!("codex/toolu/{file}")), "x")?;
  }
  symlink(
    sb.path("codex/toolu/pre-tools.d/a@t__keep.js"),
    sb.path("codex/toolu/post-tools.d/b@t__link.js"),
  )
  .map_err(|err| err.to_string())?;
  if let Some(text) = snapshot {
    write(&sb.path("codex/toolu/codex-plugins.json"), text)?;
  }
  let env = Env::from_pairs([("CODEX_HOME", sb.text("codex")), ("HOME", sb.text("home"))]);
  Ok(Roots::new(env, Some(Host::Codex)))
}

fn remaining(sb: &Sandbox) -> Res<Vec<String>> {
  let mut left = Vec::new();
  for dir in ["pre-tools.d", "post-tools.d"] {
    let read = std::fs::read_dir(sb.path(&format!("codex/toolu/{dir}")));
    for entry in read.map_err(|err| err.to_string())? {
      let name = entry.map_err(|err| err.to_string())?.file_name();
      left.push(format!("{dir}/{}", name.to_string_lossy()));
    }
  }
  left.sort();
  Ok(left)
}

#[test]
fn a_ready_snapshot_prunes_the_absent_plugins_regular_files() {
  let sb = Sandbox::new().unwrap();
  let ready = r#"{"version":1,"status":"ready","plugins":["a@t"]}"#;
  let roots = codex_home(&sb, Some(ready)).unwrap();
  let mut removed: Vec<String> = prune_inactive_modules(&roots)
    .iter()
    .map(|path| {
      path
        .strip_prefix(sb.path("codex/toolu"))
        .unwrap()
        .display()
        .to_string()
    })
    .collect();
  removed.sort();
  assert_eq!(
    removed,
    [
      "post-tools.d/b@t__rule.json",
      "pre-tools.d/b@t__drop.js",
      "pre-tools.d/b@t__drop.sh"
    ]
  );
  assert_eq!(
    remaining(&sb).unwrap(),
    [
      "post-tools.d/.b@t__hidden.sh",
      "post-tools.d/a@t__rule.json",
      "post-tools.d/b@t__link.js",
      "post-tools.d/b@t__notes.txt",
      "pre-tools.d/a@t__keep.js",
      "pre-tools.d/noname.sh",
    ]
  );
}

#[test]
fn an_untrustworthy_snapshot_or_another_host_prunes_nothing() {
  for snapshot in [
    None,
    Some(r#"{"version":1,"status":"indeterminate","plugins":[]}"#),
    Some("not json"),
  ] {
    let sb = Sandbox::new().unwrap();
    let roots = codex_home(&sb, snapshot).unwrap();
    assert_eq!(prune_inactive_modules(&roots), NONE, "{snapshot:?}");
  }
  let sb = Sandbox::new().unwrap();
  let ready = r#"{"version":1,"status":"ready","plugins":[]}"#;
  let codex = codex_home(&sb, Some(ready)).unwrap();
  let claude = Roots::new(codex.env().clone(), Some(Host::Claude));
  assert_eq!(prune_inactive_modules(&claude), NONE);
  assert_eq!(remaining(&sb).unwrap().len(), 9);
}
