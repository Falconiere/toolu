//! Muse's installed-plugin and capability listing.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};

use super::inventory::{Inventory, MuseHealth, Plugin};

const TIMEOUT: Duration = Duration::from_secs(10);
const REPOSITORY: &str = "https://github.com/falconiere/toolu";

/// One toolu plugin selected from Muse's installed listing.
pub(super) struct MuseEntry {
  /// Plugin id.
  pub id: String,
  root: PathBuf,
  enabled: bool,
  active: bool,
  version: Option<String>,
}

/// Approval state returned by inspection.
pub(super) struct MuseApproval {
  /// Overall capability approval.
  pub approval: &'static str,
  /// Hooks awaiting review.
  pub pending_hooks: Vec<String>,
}

/// Read the installed toolu plugins and inspect their runtime capabilities.
pub(super) fn collect(roots: &Roots) -> Inventory {
  let listing = invoke(roots, &["plugins", "list", "--json"]).and_then(|text| parse_listing(&text));
  let Ok(entries) = listing else {
    return Inventory {
      plugins: Vec::new(),
      warning: Some("Muse plugin inventory is unavailable".to_owned()),
    };
  };
  let mut plugins = Vec::new();
  let mut warnings = Vec::new();
  for entry in entries {
    let approval = invoke(roots, &["plugins", "inspect", &entry.id, "--json"])
      .and_then(|text| parse_inspect(&text));
    let (approval, pending_hooks) = match approval {
      Ok(result) => (result.approval, result.pending_hooks),
      Err(_) => {
        warnings.push(format!("cannot inspect Muse plugin {}", entry.id));
        ("unknown", Vec::new())
      }
    };
    plugins.push(Plugin {
      name: entry.id,
      version: entry.version,
      root: Some(entry.root),
      muse: Some(MuseHealth {
        active: entry.active,
        enabled: entry.enabled,
        approval,
        pending_hooks,
      }),
    });
  }
  Inventory {
    plugins,
    warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
  }
}

fn invoke(roots: &Roots, args: &[&str]) -> Result<String, String> {
  let mut argv = vec!["muse".to_owned()];
  argv.extend(args.iter().map(|part| (*part).to_owned()));
  let mut spec = Spec::new(argv);
  spec.env = Some(roots.env().clone());
  spec.timeout = TIMEOUT;
  spec.max_output_bytes = 1_048_576;
  let output = run(&spec).map_err(|_| "Muse CLI unavailable".to_owned())?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err("Muse CLI listing failed".to_owned());
  }
  Ok(output.stdout)
}

/// Parse Muse's installed-plugin listing, keeping packages from this repository.
pub(super) fn parse_listing(body: &str) -> Result<Vec<MuseEntry>, String> {
  let document: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
  let list = document
    .get("plugins")
    .and_then(Value::as_array)
    .ok_or("Muse listing has no plugins array")?;
  let mut selected = Vec::new();
  for item in list {
    let plugin = item.get("plugin").ok_or("Muse listing has no plugin")?;
    let id = plugin
      .get("id")
      .and_then(Value::as_str)
      .ok_or("Muse plugin has no id")?;
    let record = item.get("record").ok_or("Muse listing has no record")?;
    let root = record
      .get("cache_path")
      .and_then(Value::as_str)
      .ok_or("Muse plugin has no cache path")?;
    let root = PathBuf::from(root);
    if !toolu_package(&root, id)? {
      continue;
    }
    selected.push(MuseEntry {
      id: id.to_owned(),
      root,
      active: item
        .get("active")
        .and_then(Value::as_bool)
        .ok_or("Muse active missing")?,
      enabled: record
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or("Muse enabled missing")?,
      version: plugin
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned),
    });
  }
  Ok(selected)
}

fn toolu_package(root: &Path, id: &str) -> Result<bool, String> {
  let manifest = root.join(".claude-plugin/plugin.json");
  let text = match std::fs::read_to_string(&manifest) {
    Ok(text) => text,
    Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
    Err(error) => return Err(format!("{}: {error}", manifest.display())),
  };
  let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
  let repo = value
    .get("repository")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_ascii_lowercase();
  Ok(
    repo.trim_end_matches(".git") == REPOSITORY
      && value.get("name").and_then(Value::as_str) == Some(id),
  )
}

/// Parse one plugin's capability inspection.
pub(super) fn parse_inspect(body: &str) -> Result<MuseApproval, String> {
  let document: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
  let capabilities = document
    .get("runtime_capabilities")
    .and_then(Value::as_array)
    .ok_or("Muse inspection has no runtime capabilities")?;
  let mut pending_hooks = Vec::new();
  let mut pending_other = false;
  for capability in capabilities {
    let candidate = capability
      .get("candidate")
      .ok_or("Muse capability has no candidate")?;
    let kind = match candidate.get("kind").and_then(Value::as_str) {
      Some(kind) => kind,
      None
        if candidate
          .get("scoped_definition_id")
          .and_then(Value::as_str)
          .is_some() =>
      {
        "agent"
      }
      None => return Err("Muse capability has no kind or scoped definition".to_owned()),
    };
    let status = capability
      .get("status")
      .and_then(Value::as_str)
      .ok_or("Muse capability has no status")?;
    match status {
      "review_needed" | "trusted_disabled" if kind == "hook" => pending_hooks.push(
        candidate
          .get("capability_id")
          .and_then(Value::as_str)
          .ok_or("Muse hook has no id")?
          .to_owned(),
      ),
      "review_needed" | "trusted_disabled" => pending_other = true,
      "trusted_enabled" => {}
      _ => return Err(format!("Muse capability has unknown status {status}")),
    }
  }
  let approval = if pending_other || !pending_hooks.is_empty() {
    "needs-approval"
  } else {
    "approved"
  };
  Ok(MuseApproval {
    approval,
    pending_hooks,
  })
}

#[cfg(test)]
#[path = "tests/muse_test.rs"]
mod tests;
