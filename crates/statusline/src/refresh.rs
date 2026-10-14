//! `SessionStart` and `toolu statusline refresh`: migrate a settings command that
//! still runs the old stable path to `toolu statusline render`, then retire that
//! path (#440) on a later refresh, once no user or project settings name it. The
//! session that migrates keeps the link, since it may still run the old command.

use std::path::Path;

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::retire::{Retired, retire};

use crate::setup;

/// What the generated fallback shim held, before #440.
const SHIM: &str = "#!/bin/sh\nexec toolu statusline render \"$@\"\n";

#[derive(Debug)]
struct Refreshed {
  retired: bool,
  migrated: bool,
  notice: Option<String>,
}

/// Remove the old link and the generated shim it pointed at, unless the settings still name them.
fn retire_paths(roots: &Roots) -> Result<bool, String> {
  let generated = roots.config_root().join("toolu/statusline-render.sh");
  let retired = retire(
    roots,
    "statusline",
    "statusline",
    "statusline.sh",
    Some(&generated),
  );
  let shim = std::fs::symlink_metadata(&generated).is_ok_and(|meta| meta.is_file())
    && std::fs::read_to_string(&generated).is_ok_and(|text| text == SHIM);
  if shim {
    std::fs::remove_file(&generated)
      .map_err(|error| format!("cannot remove {}: {error}", generated.display()))?;
  }
  match retired {
    Retired::Failed(warning) => Err(warning),
    Retired::Removed => Ok(true),
    Retired::Absent | Retired::Kept => Ok(shim),
  }
}

fn refresh(roots: &Roots) -> Result<Refreshed, String> {
  let migrated = if roots.host() == Host::Claude {
    setup::migrate(roots)?
  } else {
    None
  };
  let did_migrate = migrated.is_some();
  let still_named = setup::names_old_path(roots);
  let retired = if did_migrate || still_named.is_some() {
    false
  } else {
    retire_paths(roots)?
  };
  let notice = migrated.map(|line| line.message).or(still_named);
  Ok(Refreshed {
    retired,
    migrated: did_migrate,
    notice,
  })
}

/// Run `toolu statusline refresh` from a CLI shell.
pub(crate) fn run(roots: &Roots, json_out: bool) -> Outcome {
  match refresh(roots) {
    Ok(done) => {
      let status = if done.migrated { "WIRED" } else { "REFRESHED" };
      let text = if json_out {
        json!({
          "status": status,
          "retired": done.retired,
          "migrated": done.migrated
        })
        .to_string()
      } else if done.migrated {
        "WIRED statusline settings migrated to toolu statusline render".to_owned()
      } else {
        format!("{status} statusline settings checked")
      };
      Outcome::data(text)
    }
    Err(error) => {
      let text = if json_out {
        json!({"status":"ERROR", "retired":false, "migrated":false, "message":error}).to_string()
      } else {
        format!("ERROR {error}")
      };
      Outcome {
        exit: Exit::Failure,
        stdout: Some(text),
        stderr: None,
      }
    }
  }
}

/// The host hook says only what changed or still needs the user, and reports any failure.
pub(crate) fn session_start(roots: &Roots, _plugin_root: Option<&Path>) -> Outcome {
  match refresh(roots) {
    Ok(done) => Outcome {
      exit: Exit::Success,
      stdout: done
        .notice
        .map(|notice| json!({"systemMessage":notice}).to_string()),
      stderr: None,
    },
    Err(error) => Outcome::data(
      json!({
        "systemMessage": format!("statusline: {error}")
      })
      .to_string(),
    ),
  }
}

#[cfg(test)]
#[path = "tests/refresh_test.rs"]
mod tests;
