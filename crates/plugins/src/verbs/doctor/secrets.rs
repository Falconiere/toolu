//! secrets.json health and env credentials required by installed plugins.

use std::collections::BTreeSet;
use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::config::secrets::{self, SecretError};
use toolu_runtime::env::Env;
use toolu_runtime::host::muse;
use toolu_runtime::host::roots::Roots;

use super::{Finding, Scope};

struct Need {
  plugin: &'static str,
  variable: &'static str,
  summary: &'static str,
  fix: &'static str,
}

const NEEDS: &[Need] = &[
  Need {
    plugin: "code-review",
    variable: "OPENROUTER_API_KEY",
    summary: "unset; code-review needs it for native review",
    fix: "export OPENROUTER_API_KEY=… (https://openrouter.ai/keys)",
  },
  Need {
    plugin: "jev",
    variable: "TYPESAFE_API_KEY",
    summary: "unset; jev needs it for routing",
    fix: "export TYPESAFE_API_KEY=… (https://console.typesafe.ai/settings/keys)",
  },
  Need {
    plugin: "statusline",
    variable: "TYPESAFE_API_KEY",
    summary: "unset; statusline needs it for Jev readiness",
    fix: "export TYPESAFE_API_KEY=… (https://console.typesafe.ai/settings/keys)",
  },
  Need {
    plugin: "pr-babysit",
    variable: "TYPESAFE_API_KEY",
    summary: "unset; pr-babysit needs it for Jev routing",
    fix: "export TYPESAFE_API_KEY=… (https://console.typesafe.ai/settings/keys)",
  },
  Need {
    plugin: "epic-orchestrator",
    variable: "TYPESAFE_API_KEY",
    summary: "unset; epic-orchestrator uses it for optional Jev routing",
    fix: "export TYPESAFE_API_KEY=… (https://console.typesafe.ai/settings/keys)",
  },
];

/// secrets.json plus env presence for `installed` plugins. Never includes values.
pub(super) fn check(roots: &Roots, env: &Env, installed: &BTreeSet<String>) -> Vec<Finding> {
  let mut findings = Vec::new();
  findings.extend(file(roots));
  findings.extend(env_keys(env, installed));
  if findings.is_empty() {
    findings.push(Finding::ok(
      Scope::Secrets,
      None,
      "secrets.json readable; required env keys present",
    ));
  }
  findings
}

fn file(roots: &Roots) -> Vec<Finding> {
  match secrets::load(roots) {
    Ok(_secrets) => Vec::new(),
    Err(_error) if sandboxed_hide(roots) => {
      vec![Finding::warn(
        Scope::Secrets,
        Some("secrets.json".to_owned()),
        "unknown inside the Muse sandbox; run toolu plugins doctor from a terminal",
        None,
      )]
    }
    Err(error) => {
      let path = secrets::path(roots);
      let fix = fix_for(&error, &path);
      vec![Finding::fail(
        Scope::Secrets,
        Some("secrets.json".to_owned()),
        error.to_string(),
        Some(fix),
      )]
    }
  }
}

fn fix_for(error: &SecretError, path: &Path) -> String {
  let text = error.to_string();
  if text.contains("unsafe permissions") {
    return format!("chmod 600 {}", path.display());
  }
  if text.contains("invalid") {
    return format!("fix or recreate {}", path.display());
  }
  format!("ensure {} is readable", path.display())
}

fn env_keys(env: &Env, installed: &BTreeSet<String>) -> Vec<Finding> {
  let mut seen = BTreeSet::new();
  let mut findings = Vec::new();
  for need in NEEDS {
    if !installed.contains(need.plugin) || !seen.insert(need.variable) {
      continue;
    }
    let set = env
      .get(need.variable)
      .is_some_and(|value| !value.is_empty());
    if set {
      continue;
    }
    // Several plugins share TYPESAFE_API_KEY; name every installed one that needs it.
    let plugins: Vec<&str> = NEEDS
      .iter()
      .filter(|row| row.variable == need.variable && installed.contains(row.plugin))
      .map(|row| row.plugin)
      .collect();
    let summary = if plugins.len() == 1 {
      need.summary.to_owned()
    } else {
      format!("unset; {} need it", plugins.join(", "))
    };
    findings.push(Finding::warn(
      Scope::Secrets,
      Some(need.variable.to_owned()),
      summary,
      Some(need.fix.to_owned()),
    ));
  }
  findings
}

fn sandboxed_hide(roots: &Roots) -> bool {
  if roots.host() != Host::Muse {
    return false;
  }
  let dir = muse::config_dir(roots.env());
  if !secrets::path(roots).starts_with(&dir) {
    return false;
  }
  muse::dir_unreadable(&dir) || muse::file_unreadable(&dir.join("settings.json"))
}

#[cfg(test)]
#[path = "tests/secrets_test.rs"]
mod tests;
