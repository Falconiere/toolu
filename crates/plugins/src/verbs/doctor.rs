//! `toolu plugins doctor`: plugin lifecycle health per host, with fix commands.

use toolu_protocol::host::Host;
use toolu_runtime::config::secrets::{self as epic_secrets, Secrets};
use toolu_runtime::host::roots::Roots;

use crate::args::Request;
use crate::catalog::Catalog;
use crate::dispatch::{Context, Report};
use crate::fail::Res;
use crate::host::{Target, none_found, with_toolu};

mod codex_pin;
mod format;
mod host;
mod plugin;
mod secrets;

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
  /// Healthy.
  Ok,
  /// Actionable, non-blocking.
  Warn,
  /// Blocking.
  Fail,
}

impl Level {
  fn name(self) -> &'static str {
    match self {
      Level::Ok => "ok",
      Level::Warn => "warn",
      Level::Fail => "fail",
    }
  }
}

/// What the finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
  /// Host inventory or marketplace setup.
  Host,
  /// One installed plugin.
  Plugin,
  /// Catalog dependency gap.
  Deps,
  /// secrets.json or env credentials.
  Secrets,
}

impl Scope {
  fn name(self) -> &'static str {
    match self {
      Scope::Host => "host",
      Scope::Plugin => "plugin",
      Scope::Deps => "deps",
      Scope::Secrets => "secrets",
    }
  }
}

/// One diagnostic line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
  /// Severity.
  pub(crate) status: Level,
  /// Kind.
  pub(crate) scope: Scope,
  /// Plugin or secret name when relevant.
  pub(crate) name: Option<String>,
  /// Human summary; never a secret value.
  pub(crate) summary: String,
  /// Exact command or hint when something is wrong.
  pub(crate) fix: Option<String>,
}

impl Finding {
  fn ok(scope: Scope, name: Option<String>, summary: impl Into<String>) -> Finding {
    Finding {
      status: Level::Ok,
      scope,
      name,
      summary: summary.into(),
      fix: None,
    }
  }

  fn warn(
    scope: Scope,
    name: Option<String>,
    summary: impl Into<String>,
    fix: Option<String>,
  ) -> Finding {
    Finding {
      status: Level::Warn,
      scope,
      name,
      summary: summary.into(),
      fix,
    }
  }

  fn fail(
    scope: Scope,
    name: Option<String>,
    summary: impl Into<String>,
    fix: Option<String>,
  ) -> Finding {
    Finding {
      status: Level::Fail,
      scope,
      name,
      summary: summary.into(),
      fix,
    }
  }
}

/// One host's findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostReport {
  /// The host.
  pub(crate) host: Target,
  /// Its findings in report order.
  pub(crate) findings: Vec<Finding>,
}

/// Totals across every finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Counts {
  ok: usize,
  warn: usize,
  fail: usize,
}

impl Counts {
  fn add(&mut self, level: Level) {
    match level {
      Level::Ok => self.ok += 1,
      Level::Warn => self.warn += 1,
      Level::Fail => self.fail += 1,
    }
  }

  fn of(sections: &[HostReport]) -> Counts {
    let mut counts = Counts::default();
    for section in sections {
      for finding in &section.findings {
        counts.add(finding.status);
      }
    }
    counts
  }

  fn line(self) -> String {
    format!("{} ok, {} warn, {} fail", self.ok, self.warn, self.fail)
  }
}

/// Diagnoses every selected host.
pub(crate) fn run(
  request: &Request,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  let targets = if let Some(target) = wanted {
    vec![target]
  } else {
    let found = with_toolu(&cx.env);
    if found.is_empty() {
      return Err(none_found());
    }
    found
  };
  let sections: Vec<HostReport> = targets
    .into_iter()
    .map(|target| host::diagnose(target, catalog, request, cx))
    .collect();
  let counts = Counts::of(&sections);
  let failed = counts.fail > 0;
  let hosts: Vec<Host> = sections
    .iter()
    .map(|section| host_of(section.host))
    .collect();
  let body = if cx.style.rails() {
    format::rails(cx, &sections, counts)
  } else {
    format::plain(&sections, counts)
  };
  Ok(Report {
    text: redact_text_all(&body, &cx.env, &hosts),
    json: redact_json_all(&format::document(&sections, counts), &cx.env, &hosts),
    failed,
  })
}

fn redact_text_all(text: &str, env: &toolu_runtime::env::Env, hosts: &[Host]) -> String {
  let mut out = text.to_owned();
  for host in hosts {
    out = epic_secrets::redact_text(&out, &load_secrets(env, Some(*host)));
  }
  out
}

fn redact_json_all(
  value: &serde_json::Value,
  env: &toolu_runtime::env::Env,
  hosts: &[Host],
) -> serde_json::Value {
  let mut out = value.clone();
  for host in hosts {
    out = epic_secrets::redact_json(&out, &load_secrets(env, Some(*host)));
  }
  out
}

fn load_secrets(env: &toolu_runtime::env::Env, host: Option<Host>) -> Secrets {
  let roots = Roots::new(env.clone(), host);
  epic_secrets::load(&roots).unwrap_or_else(|_| Secrets::none())
}

/// The protocol host for an installer target.
pub(super) fn host_of(target: Target) -> Host {
  match target {
    Target::Claude => Host::Claude,
    Target::Codex => Host::Codex,
    Target::Muse => Host::Muse,
    Target::Cursor => Host::Cursor,
  }
}

#[cfg(test)]
#[path = "tests/doctor_test.rs"]
mod tests;
