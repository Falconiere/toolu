//! The native secrets scanner: the shared known-token table plus an entropy check.
//! Findings come from the `New` side of the shaped diff; redactions cover both
//! sides. The table is the xtask guardrail's own `rules.json`, read at compile
//! time, so there is one source of patterns.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

use super::{Notice, NoticeKind, Redaction, finding_id};
use crate::contract::{Finding, RedactionSpan, Severity, Side, Source};
use crate::shape::Shaped;

/// The guardrail's rule table; only its `secrets` array is read here.
const TABLE: &str = include_str!("../../../../tooling/conventions/guardrails/rust/rules.json");
/// Bits per character from which a token that matches no pattern looks random.
const ENTROPY_MIN: f64 = 4.5;
/// A candidate token: a run of base64, hex, URL-safe or dotted characters.
const TOKEN: &str = r"[A-Za-z0-9+/=_\-.]{20,}";
/// What replaces a secret in a quote.
const MASK: &str = "***";

#[derive(Deserialize)]
struct Table {
  secrets: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
  id: String,
  regex: String,
}

struct Known {
  id: String,
  regex: Regex,
}

/// The compiled table and the entropy token pattern.
struct Scanner {
  known: Vec<Known>,
  token: Regex,
}

/// What one line holds.
struct Hit {
  category: String,
  claim: String,
  severity: Severity,
  spans: Vec<RedactionSpan>,
}

impl Scanner {
  fn compile() -> Result<Scanner, String> {
    let table: Table =
      serde_json::from_str(TABLE).map_err(|err| format!("secrets table: {err}"))?;
    let mut known = Vec::new();
    for entry in table.secrets {
      let regex = Regex::new(&entry.regex).map_err(|err| format!("secret {}: {err}", entry.id))?;
      known.push(Known {
        id: entry.id,
        regex,
      });
    }
    let token = Regex::new(TOKEN).map_err(|err| format!("entropy token: {err}"))?;
    Ok(Scanner { known, token })
  }

  /// Every known match and every high-entropy token of `text`, so a second secret
  /// on the line is masked too.
  fn secret_spans(&self, text: &str) -> Vec<RedactionSpan> {
    let mut all: Vec<RedactionSpan> = self
      .known
      .iter()
      .flat_map(|rule| spans(&rule.regex, text, |_| true))
      .collect();
    all.extend(spans(&self.token, text, |token| {
      entropy(token) >= ENTROPY_MIN
    }));
    merge(all)
  }

  /// Category, claim and severity come from the first matching known pattern, else
  /// from entropy alone; `None` for a line without a secret.
  fn inspect(&self, text: &str) -> Option<Hit> {
    let spans = self.secret_spans(text);
    if spans.is_empty() {
      return None;
    }
    let Some(first) = self.known.iter().find(|rule| rule.regex.is_match(text)) else {
      return Some(Hit {
        category: "secret-entropy".to_owned(),
        claim: "high-entropy token looks like a secret".to_owned(),
        severity: Severity::Medium,
        spans,
      });
    };
    Some(Hit {
      category: format!("secret-{}", first.id),
      claim: format!("possible committed secret ({})", first.id),
      severity: Severity::Blocker,
      spans,
    })
  }
}

/// The compiled scanner, built once per process.
static SCANNER: OnceLock<Result<Scanner, String>> = OnceLock::new();

fn scanner() -> Result<&'static Scanner, &'static str> {
  SCANNER
    .get_or_init(Scanner::compile)
    .as_ref()
    .map_err(String::as_str)
}

/// `text` with every secret masked, and the byte spans of what was masked, for
/// quotes of findings that do not come from the secrets scan. Without a compiled
/// table nothing proves the line clean, so all of it is masked; `scan` reports why.
pub(super) fn redact_line(text: &str) -> (String, Vec<RedactionSpan>) {
  let spans = match scanner() {
    Ok(scanner) => scanner.secret_spans(text),
    Err(_) if !text.is_empty() => vec![RedactionSpan {
      start: 0,
      end: text.len(),
    }],
    Err(_) => Vec::new(),
  };
  (mask(text, &spans), spans)
}

/// Byte ranges of `regex`'s matches in `text` that `keep` accepts, in order.
fn spans(regex: &Regex, text: &str, keep: impl Fn(&str) -> bool) -> Vec<RedactionSpan> {
  regex
    .find_iter(text)
    .filter(|found| keep(found.as_str()))
    .map(|found| RedactionSpan {
      start: found.start(),
      end: found.end(),
    })
    .collect()
}

/// `spans` sorted by start, with overlapping or touching ranges joined, so the
/// result is disjoint.
fn merge(mut spans: Vec<RedactionSpan>) -> Vec<RedactionSpan> {
  spans.sort_by_key(|span| (span.start, span.end));
  let mut merged: Vec<RedactionSpan> = Vec::new();
  for span in spans {
    match merged.last_mut() {
      Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
      _ => merged.push(span),
    }
  }
  merged
}

fn real(count: usize) -> f64 {
  f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Shannon entropy of `token`, in bits per byte.
fn entropy(token: &str) -> f64 {
  let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
  for byte in token.bytes() {
    *counts.entry(byte).or_default() += 1;
  }
  let total = real(token.len());
  counts
    .values()
    .map(|count| {
      let share = real(*count) / total;
      -share * share.log2()
    })
    .sum()
}

/// `text` with each span replaced by `***`; spans are sorted and disjoint.
fn mask(text: &str, spans: &[RedactionSpan]) -> String {
  let mut masked = String::new();
  let mut at = 0;
  for span in spans {
    masked.push_str(text.get(at..span.start).unwrap_or_default());
    masked.push_str(MASK);
    at = span.end;
  }
  masked.push_str(text.get(at..).unwrap_or_default());
  masked
}

/// Secrets findings for the `New` lines of `shaped`, a redaction for every changed
/// line on either side that holds a secret, and a notice when the table cannot be
/// compiled (then no secrets run, loudly).
pub(super) fn scan(shaped: &Shaped) -> (Vec<Finding>, Vec<Redaction>, Vec<Notice>) {
  let scanner = match scanner() {
    Ok(scanner) => scanner,
    Err(why) => {
      let notice = Notice {
        kind: NoticeKind::SecretsCompile,
        text: format!("prepass: secrets scan skipped: {why}"),
      };
      return (Vec::new(), Vec::new(), vec![notice]);
    }
  };
  let mut findings = Vec::new();
  let mut redactions = Vec::new();
  for file in &shaped.files {
    for line in &file.lines {
      let Some(hit) = scanner.inspect(&line.text) else {
        continue;
      };
      if line.side == Side::New {
        findings.push(Finding {
          id: finding_id(Source::Secrets, &file.path, line.line, &hit.category),
          path: file.path.clone(),
          line: line.line,
          end_line: line.line,
          side: Side::New,
          severity: hit.severity,
          category: hit.category,
          source: Source::Secrets,
          claim: hit.claim,
          quote: mask(&line.text, &hit.spans),
          suggestion: None,
          redaction_spans: hit.spans.clone(),
        });
      }
      redactions.push(Redaction {
        path: file.path.clone(),
        side: line.side,
        line: line.line,
        spans: hit.spans,
      });
    }
  }
  (findings, redactions, Vec::new())
}

#[cfg(test)]
#[path = "tests/secrets_test.rs"]
mod tests;
