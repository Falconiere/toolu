//! `SessionStart`: keep the old statusline path alive and migrate it once to the native command.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::publish::{PublishOptions, publish};
use toolu_runtime::startup::report::HelperStatus;

use crate::setup;

const SHIM: &str = "#!/bin/sh\nexec toolu statusline render \"$@\"\n";

#[derive(Debug)]
struct Refreshed {
  published: bool,
  migrated: bool,
  notice: Option<String>,
}

fn generated_source(roots: &Roots) -> Result<PathBuf, String> {
  let dir = roots.config_root().join("toolu");
  std::fs::create_dir_all(&dir)
    .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
  let path = dir.join("statusline-render.sh");
  let opened = OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o755)
    .open(&path);
  match opened {
    Ok(mut file) => {
      if let Err(error) = file.write_all(SHIM.as_bytes()) {
        let _removed = std::fs::remove_file(&path);
        return Err(format!("cannot write {}: {error}", path.display()));
      }
      Ok(path)
    }
    Err(error) if error.kind() == ErrorKind::AlreadyExists => {
      let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
      let executable = std::fs::metadata(&path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
      if text == SHIM && executable {
        Ok(path)
      } else {
        Err(format!(
          "{} is not the toolu statusline shim",
          path.display()
        ))
      }
    }
    Err(error) => Err(format!("cannot create {}: {error}", path.display())),
  }
}

fn source(roots: &Roots, plugin_root: Option<&Path>) -> Result<PathBuf, String> {
  if let Some(root) = plugin_root {
    return Ok(root.join("scripts/statusline.sh"));
  }
  generated_source(roots)
}

fn published(roots: &Roots, source: &Path) -> Result<bool, String> {
  let done = publish(&PublishOptions {
    plugin: "statusline",
    source,
    dir: "statusline",
    name: "statusline.sh",
    what: "renderer",
    roots,
  });
  if let Some(error) = done.report_error {
    return Err(error);
  }
  if let Some(warning) = done.warning {
    return Err(warning);
  }
  match done.result.status {
    HelperStatus::Published => Ok(true),
    HelperStatus::KeptUserFile => Ok(false),
    HelperStatus::LinkFailed | HelperStatus::Unwritable | HelperStatus::SourceMissing => {
      Err("statusline compatibility shim could not be published".to_owned())
    }
  }
}

fn refresh(roots: &Roots, plugin_root: Option<&Path>) -> Result<Refreshed, String> {
  let shim = source(roots, plugin_root)?;
  let published = published(roots, &shim)?;
  let migrated = if roots.host() == Host::Claude {
    setup::migrate(roots)?
  } else {
    None
  };
  Ok(Refreshed {
    published,
    migrated: migrated.is_some(),
    notice: migrated.map(|line| line.message),
  })
}

/// Run `toolu statusline refresh` from a CLI shell.
pub(crate) fn run(roots: &Roots, json_out: bool) -> Outcome {
  match refresh(roots, None) {
    Ok(done) => {
      let status = if done.migrated { "WIRED" } else { "REFRESHED" };
      let text = if json_out {
        json!({
          "status": status,
          "published": done.published,
          "migrated": done.migrated
        })
        .to_string()
      } else if done.migrated {
        "WIRED statusline settings migrated; compatibility path refreshed".to_owned()
      } else {
        format!("{status} statusline compatibility path refreshed")
      };
      Outcome::data(text)
    }
    Err(error) => {
      let text = if json_out {
        json!({"status":"ERROR", "published":false, "migrated":false, "message":error}).to_string()
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

/// The host hook is silent after migration and reports any failure as context.
pub(crate) fn session_start(roots: &Roots, plugin_root: Option<&Path>) -> Outcome {
  match refresh(roots, plugin_root) {
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
