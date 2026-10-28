//! Masks every pre-pass secret (#664) in outbound text: the prompt, Jev state and
//! recheck messages. The spans index the unmasked changed lines, so each secret's
//! text is looked up there once and then replaced wherever it appears, context
//! lines and source included.

use std::cmp::Reverse;

use crate::prepass::Redaction;
use crate::shape::Shaped;

/// What a secret becomes in outbound text.
pub(crate) const MASK: &str = "[REDACTED]";

/// The secret texts of one run, longest first so a secret containing another is
/// masked whole.
#[derive(Debug, Default)]
pub(crate) struct Mask(Vec<String>);

impl Mask {
  /// The mask for `redactions` over `shaped`'s changed lines.
  ///
  /// # Errors
  /// A redaction whose line or span is not in `shaped`: its secret cannot be
  /// found, so nothing may be sent.
  pub(crate) fn new(shaped: &Shaped, redactions: &[Redaction]) -> Result<Mask, String> {
    let mut secrets = Vec::new();
    for redaction in redactions {
      let line = shaped
        .files
        .iter()
        .filter(|file| file.path == redaction.path)
        .flat_map(|file| &file.lines)
        .find(|line| line.side == redaction.side && line.line == redaction.line)
        .ok_or_else(|| unresolved(redaction))?;
      for span in &redaction.spans {
        let secret = line
          .text
          .get(span.start..span.end)
          .filter(|secret| !secret.is_empty())
          .ok_or_else(|| unresolved(redaction))?;
        secrets.push(secret.to_owned());
      }
    }
    secrets.sort_by_key(|secret| Reverse(secret.len()));
    secrets.dedup();
    Ok(Mask(secrets))
  }

  /// `text` with every secret replaced by `MASK`.
  pub(crate) fn apply(&self, text: &str) -> String {
    self.0.iter().fold(text.to_owned(), |masked, secret| {
      masked.replace(secret.as_str(), MASK)
    })
  }
}

fn unresolved(redaction: &Redaction) -> String {
  format!(
    "secret span at {}:{} is not in the shaped diff; nothing was sent",
    redaction.path, redaction.line
  )
}

#[cfg(test)]
#[path = "tests/mask_test.rs"]
mod tests;
