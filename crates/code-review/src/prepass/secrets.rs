//! The native secrets scanner: the shared known-token table plus an entropy check,
//! over the `New` side of the shaped diff. The table is the xtask guardrail's own
//! `rules.json`, read at compile time, so there is one source of patterns.

use std::collections::BTreeMap;

use regex::Regex;
use serde::Deserialize;

use super::{Notice, NoticeKind, finding_id};
use crate::contract::{Finding, RedactionSpan, Severity, Side, Source};
use crate::shape::Shaped;

/// The guardrail's rule table; only its `secrets` array is read here.
const TABLE: &str = include_str!("../../../../tooling/conventions/guardrails/rust/rules.json");
/// What the secrets findings call themselves in ids.
const TAG: &str = "secrets";
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

  /// A known pattern wins; only a line without one is checked for entropy.
  fn inspect(&self, text: &str) -> Option<Hit> {
    self.known_hit(text).or_else(|| self.entropy_hit(text))
  }

  /// Category, claim and severity come from the first matching pattern; the spans
  /// come from every known pattern, so a second secret on the line is masked too.
  fn known_hit(&self, text: &str) -> Option<Hit> {
    let first = self.known.iter().find(|rule| rule.regex.is_match(text))?;
    let all = self
      .known
      .iter()
      .flat_map(|rule| spans(&rule.regex, text, |_| true))
      .collect();
    Some(Hit {
      category: format!("secret-{}", first.id),
      claim: format!("possible committed secret ({})", first.id),
      severity: Severity::Blocker,
      spans: merge(all),
    })
  }

  fn entropy_hit(&self, text: &str) -> Option<Hit> {
    let spans = spans(&self.token, text, |token| entropy(token) >= ENTROPY_MIN);
    (!spans.is_empty()).then(|| Hit {
      category: "secret-entropy".to_owned(),
      claim: "high-entropy token looks like a secret".to_owned(),
      severity: Severity::Medium,
      spans,
    })
  }
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

/// Secrets findings for the `New` lines of `shaped`, and a notice when the table
/// cannot be compiled (then no secrets run, loudly).
pub(super) fn scan(shaped: &Shaped) -> (Vec<Finding>, Vec<Notice>) {
  let scanner = match Scanner::compile() {
    Ok(scanner) => scanner,
    Err(why) => {
      let notice = Notice {
        kind: NoticeKind::SecretsCompile,
        text: format!("prepass: secrets scan skipped: {why}"),
      };
      return (Vec::new(), vec![notice]);
    }
  };
  let findings = shaped
    .files
    .iter()
    .flat_map(|file| {
      let scanner = &scanner;
      file
        .lines
        .iter()
        .filter(|line| line.side == Side::New)
        .filter_map(move |line| {
          let hit = scanner.inspect(&line.text)?;
          Some(Finding {
            id: finding_id(TAG, &file.path, line.line, &hit.category),
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
            redaction_spans: hit.spans,
          })
        })
    })
    .collect();
  (findings, Vec::new())
}

#[cfg(test)]
#[path = "tests/secrets_test.rs"]
mod tests;
