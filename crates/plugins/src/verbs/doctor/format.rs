//! Text, rails and JSON rendering for `plugins doctor`.

use std::fmt::Write as _;

use serde_json::{Map, Value, json};
use toolu_runtime::term::{Tone, intro, outro, paint, row, section};

use super::{Counts, Finding, HostReport, Level};
use crate::dispatch::Context;

/// Plain text report ending in the summary counts.
pub(super) fn plain(sections: &[HostReport], counts: Counts) -> String {
  let body = if let [only] = sections {
    host_plain(only)
  } else {
    sections
      .iter()
      .map(|report| format!("{}:\n{}", report.host.name(), host_plain(report)))
      .collect::<Vec<_>>()
      .join("\n")
  };
  format!("{body}{}\n", counts.line())
}

fn host_plain(report: &HostReport) -> String {
  report
    .findings
    .iter()
    .fold(String::new(), |mut out, finding| {
      let name = finding.name.as_deref().unwrap_or("-");
      let fix = finding
        .fix
        .as_deref()
        .map(|fix| format!("; {fix}"))
        .unwrap_or_default();
      let _ = writeln!(
        out,
        "  {:<4} {:<8} {name:<16} {}{fix}",
        finding.status.name(),
        finding.scope.name(),
        finding.summary
      );
      out
    })
}

/// Rails report for a terminal.
pub(super) fn rails(cx: &Context<'_>, sections: &[HostReport], counts: Counts) -> String {
  let style = cx.style;
  let width = sections
    .iter()
    .flat_map(|report| {
      report
        .findings
        .iter()
        .map(|finding| finding.name.as_deref().unwrap_or("-").chars().count())
    })
    .max()
    .unwrap_or(0);
  let mut text = if cx.progress.is_none() {
    intro(style, "toolu plugins doctor")
  } else {
    String::new()
  };
  for (index, report) in sections.iter().enumerate() {
    if index > 0 {
      let _ = writeln!(text, "{}", paint(style, Tone::Dim, "│"));
    }
    text.push_str(&section(style, report.host.name()));
    for finding in &report.findings {
      let name = finding.name.as_deref().unwrap_or("-");
      let detail = match &finding.fix {
        Some(fix) => format!("{} · {fix}", finding.summary),
        None => finding.summary.clone(),
      };
      text.push_str(&row(style, mark(finding.status), name, width, &detail));
    }
  }
  let secs = cx.started.elapsed().as_secs_f64();
  text.push_str(&outro(style, &format!("{}  ({secs:.1}s)", counts.line())));
  text
}

fn mark(level: Level) -> (&'static str, Tone) {
  match level {
    Level::Ok => ("✔", Tone::Good),
    Level::Warn => ("▲", Tone::Warn),
    Level::Fail => ("✘", Tone::Bad),
  }
}

/// `--json` document.
pub(super) fn document(sections: &[HostReport], counts: Counts) -> Value {
  let hosts: Vec<Value> = sections.iter().map(host_json).collect();
  json!({
    "namespace": "plugins",
    "verb": "doctor",
    "ok": counts.fail == 0,
    "hosts": hosts,
    "counts": {
      "ok": counts.ok,
      "warn": counts.warn,
      "fail": counts.fail,
    }
  })
}

fn host_json(report: &HostReport) -> Value {
  let findings: Vec<Value> = report.findings.iter().map(finding_json).collect();
  json!({ "host": report.host.name(), "findings": findings })
}

fn finding_json(finding: &Finding) -> Value {
  let mut doc = Map::new();
  doc.insert("status".to_owned(), json!(finding.status.name()));
  doc.insert("scope".to_owned(), json!(finding.scope.name()));
  doc.insert("name".to_owned(), json!(finding.name));
  doc.insert("summary".to_owned(), json!(finding.summary));
  doc.insert("fix".to_owned(), json!(finding.fix));
  Value::Object(doc)
}

#[cfg(test)]
#[path = "tests/format_test.rs"]
mod tests;
