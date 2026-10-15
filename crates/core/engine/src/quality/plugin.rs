//! What a quality plugin's native `SessionStart` hooks share: the manifest that
//! enables its compiled-in rule, the removal of the plugin's older `.js` module
//! and `.sh` bridge (#440: nothing reads them any more), and the Codex warning
//! when the toolu core is missing.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, SystemTime};

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::atomic::replace_atomic;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::manifest::ModuleManifest;
use toolu_runtime::registry::{ModuleKind, RegistryEvent, event_dir, file_name};
use toolu_runtime::startup::dependencies::{NoticeStyle, codex_dependency_notice};

/// The core plugin every quality plugin needs.
const CORE_PLUGIN: &str = "toolu@toolu";

/// `find -mmin +1`: a concurrent `SessionStart`'s in-flight tmp is left alone.
const RESIDUE_AGE: Duration = Duration::from_secs(60);

/// One quality plugin whose post-edit rule is compiled into `toolu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityPlugin {
  /// The plugin directory and CLI namespace, such as `ts-quality`.
  pub plugin: &'static str,
  /// The registry spec of its rule, such as `ts-quality@toolu`.
  pub spec: &'static str,
  /// The manifest and rule name.
  pub name: &'static str,
}

fn has_extension(name: &str, extensions: &[&str]) -> bool {
  Path::new(name)
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| extensions.contains(&extension))
}

/// A temp file a killed writer left: the old register's `<module>.tmp.<pid>`,
/// or [`replace_atomic`]'s `<module>.<pid>.<nanoseconds>.tmp`.
fn is_residue(name: &str) -> bool {
  name.contains(".js.tmp.") || name.contains(".sh.tmp.") || has_extension(name, &["tmp"])
}

fn old_enough(path: &Path) -> bool {
  fs::symlink_metadata(path)
    .and_then(|meta| meta.modified())
    .is_ok_and(|when| {
      SystemTime::now()
        .duration_since(when)
        .is_ok_and(|age| age > RESIDUE_AGE)
    })
}

/// Write `body` to `target` atomically unless it already holds exactly that.
fn write_if_changed(target: &Path, body: &str) -> Result<(), String> {
  if fs::read(target).is_ok_and(|existing| existing == body.as_bytes()) {
    return Ok(());
  }
  replace_atomic(target, body).map_err(|error| format!("{}: {error}", target.display()))
}

impl QualityPlugin {
  fn manifest_body(&self) -> Result<String, String> {
    let manifest = ModuleManifest {
      version: 1,
      spec: self.spec.to_owned(),
      name: self.name.to_owned(),
      event: RegistryEvent::ToolPost,
      matcher: "*".to_owned(),
    };
    let body = serde_json::to_string(&manifest).map_err(|error| error.to_string())?;
    Ok(format!("{body}\n"))
  }

  /// Remove this plugin's other `.js`/`.sh` modules and aged tmp residue from
  /// `dir`; whether the old `.js` module was among them.
  fn prune(&self, dir: &Path, keep: &Path) -> Result<bool, String> {
    let Ok(list) = fs::read_dir(dir) else {
      return Ok(false);
    };
    let old = file_name(self.spec, self.name, ModuleKind::Esm).map_err(|error| error.0)?;
    let prefix = format!("{}__", self.spec);
    let mut migrated = false;
    for entry in list.flatten() {
      let name = entry.file_name().to_string_lossy().into_owned();
      let path = entry.path();
      if !name.starts_with(&prefix) || path == keep {
        continue;
      }
      let stale = if is_residue(&name) {
        old_enough(&path)
      } else {
        has_extension(&name, &["js", "sh"]) && fs::metadata(&path).is_ok_and(|meta| meta.is_file())
      };
      if stale {
        fs::remove_file(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        migrated |= name == old;
      }
    }
    Ok(migrated)
  }

  fn report(&self, env: &Env, target: &Path) -> Result<(), String> {
    let Some(report) = env.get("TOOLU_STARTUP_REPORT") else {
      return Ok(());
    };
    let record = serde_json::json!({
      "kind": "native-registry",
      "spec": self.spec,
      "name": self.name,
      "event": RegistryEvent::ToolPost,
      "matcher": "*",
      "target": target,
    });
    let mut line = record.to_string().into_bytes();
    line.push(b'\n');
    OpenOptions::new()
      .append(true)
      .open(report)
      .and_then(|mut output| output.write_all(&line))
      .map_err(|error| format!("cannot write startup report {report}: {error}"))
  }

  fn sync(&self, env: &Env, roots: &Roots) -> Result<bool, String> {
    let post = event_dir(roots, RegistryEvent::ToolPost);
    fs::create_dir_all(&post).map_err(|error| format!("{}: {error}", post.display()))?;
    let named = |kind| file_name(self.spec, self.name, kind).map_err(|error| error.0);
    let manifest = post.join(named(ModuleKind::Manifest)?);
    write_if_changed(&manifest, &self.manifest_body()?)?;
    let pre = event_dir(roots, RegistryEvent::ToolPre);
    let migrated = self.prune(&post, &manifest)? | self.prune(&pre, &manifest)?;
    self.report(env, &manifest)?;
    Ok(migrated)
  }

  /// Publish the manifest, and remove this plugin's old `.js` module and `.sh`
  /// bridge. Never fails the session: an error is a `systemMessage`.
  pub fn register(&self, env: &Env, host: Host) -> Outcome {
    let roots = Roots::new(env.clone(), Some(host));
    let message = match self.sync(env, &roots) {
      Ok(true) => Some(format!(
        "{}'s JavaScript module was replaced by its native rule in toolu.",
        self.plugin
      )),
      Ok(false) => None,
      Err(error) => Some(format!("{} registration failed: {error}", self.plugin)),
    };
    Outcome {
      exit: Exit::Success,
      stdout: message.map(|text| serde_json::json!({ "systemMessage": text }).to_string()),
      stderr: None,
    }
  }
}

/// A quality plugin's `check-toolu` hook: the Codex warning when the toolu core is
/// not installed; silent on hosts that resolve `plugin.json` dependencies themselves.
pub fn check_toolu(env: &Env) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let notice = codex_dependency_notice(&[CORE_PLUGIN], NoticeStyle::Core, &roots);
  Outcome {
    exit: Exit::Success,
    stdout: notice.map(|line| line.trim_end_matches('\n').to_owned()),
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/plugin_test.rs"]
mod tests;
