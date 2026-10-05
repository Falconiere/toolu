use std::path::Path;

use super::{Binary, Caller, Skew, assess};
use crate::manifest::Manifest;

const UPGRADE: &str = "brew upgrade toolu";

fn judge(binary: (&str, u32), plugin: (&str, u32)) -> Skew {
  let caller = Caller {
    plugin: "toolu",
    root: Path::new("/plugins/toolu"),
  };
  let binary = Binary {
    version: binary.0,
    protocol: binary.1,
  };
  let manifest = Ok(Manifest {
    version: plugin.0.to_owned(),
    hook_protocol: plugin.1,
  });
  assess(&caller, &binary, &manifest, UPGRADE)
}

fn advice(skew: Skew) -> String {
  match skew {
    Skew::Advise(text) => text,
    Skew::Same | Skew::Mismatch(_) => panic!("expected advice, got {skew:?}"),
  }
}

fn mismatch(skew: Skew) -> String {
  match skew {
    Skew::Mismatch(text) => text,
    Skew::Same | Skew::Advise(_) => panic!("expected a mismatch, got {skew:?}"),
  }
}

#[test]
fn the_same_version_and_protocol_run_silently() {
  assert_eq!(judge(("7.10.0", 1), ("7.10.0", 1)), Skew::Same);
}

#[test]
fn an_older_binary_advises_the_upgrade_command() {
  assert_eq!(
    advice(judge(("8.1.0", 1), ("8.3.0", 1))),
    "toolu 8.1.0 is older than the toolu plugin 8.3.0; upgrade it: brew upgrade toolu"
  );
  assert!(advice(judge(("7.10.0", 1), ("8.0.0", 1))).contains("is older than"));
}

#[test]
fn a_newer_differing_or_unparsable_binary_advises_a_plugin_update() {
  let newer = advice(judge(("7.11.0", 1), ("7.10.0", 1)));
  assert_eq!(
    newer,
    "toolu 7.11.0 is newer than the toolu plugin 7.10.0; \
     update the plugins from your host's marketplace"
  );
  assert!(advice(judge(("7.10.0", 1), ("7.10.0-rc.1", 1))).contains("does not match"));
  assert!(advice(judge(("7.10.0", 1), ("", 1))).contains("cannot be compared with"));
}

#[test]
fn a_higher_plugin_protocol_needs_a_newer_binary() {
  assert_eq!(
    mismatch(judge(("8.1.0", 1), ("8.3.0", 2))),
    "toolu plugin: hook protocol 2 needs a newer toolu - toolu 8.1.0 speaks protocol 1; \
     upgrade it: brew upgrade toolu"
  );
}

#[test]
fn a_lower_plugin_protocol_needs_newer_plugins() {
  let text = mismatch(judge(("9.0.0", 2), ("8.3.0", 1)));
  assert_eq!(
    text,
    "toolu plugin: hook protocol 1 is older than toolu 9.0.0 - protocol 2; \
     update the plugins from your host's marketplace"
  );
}

#[test]
fn an_unreadable_manifest_is_a_mismatch_naming_the_root_and_reason() {
  let caller = Caller {
    plugin: "jev",
    root: Path::new("/p/jev"),
  };
  let binary = Binary {
    version: "7.10.0",
    protocol: 1,
  };
  let text = mismatch(assess(
    &caller,
    &binary,
    &Err("no file".to_owned()),
    UPGRADE,
  ));
  assert_eq!(
    text,
    "jev plugin: cannot read hookProtocol from /p/jev: no file; \
     update the plugins from your host's marketplace"
  );
}
