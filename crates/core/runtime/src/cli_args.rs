//! Argument helpers clap does not provide (#442 replaced `flagValue` and
//! `CliExit`): a number as `jq --argjson` read one (`numberValue` in
//! `packages/toolu-core/src/cli/cli.ts`).

/// Whether `text` is `[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?` with ASCII digits.
pub(crate) fn is_number_text(text: &str) -> bool {
  let bytes = text.as_bytes();
  let mut at = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
  let whole = digits(bytes, at);
  at += whole;
  let mut fraction = 0;
  let dot = bytes.get(at) == Some(&b'.');
  if dot {
    at += 1;
    fraction = digits(bytes, at);
    at += fraction;
  }
  if whole == 0 && fraction == 0 {
    return false;
  }
  if matches!(bytes.get(at), Some(b'e' | b'E')) {
    at += 1;
    at += usize::from(matches!(bytes.get(at), Some(b'+' | b'-')));
    let exponent = digits(bytes, at);
    if exponent == 0 {
      return false;
    }
    at += exponent;
  }
  at == bytes.len()
}

fn digits(bytes: &[u8], from: usize) -> usize {
  let rest = bytes.get(from..).unwrap_or_default();
  rest.iter().take_while(|byte| byte.is_ascii_digit()).count()
}

/// `text` as a finite number, allowing ASCII whitespace around it, a leading
/// `+` or `0` and a trailing `.`; `None` for anything else, `1e999` included.
pub fn jq_number(text: &str) -> Option<f64> {
  let trimmed = text.trim_matches([' ', '\t', '\n', '\r']);
  if !is_number_text(trimmed) {
    return None;
  }
  trimmed
    .parse::<f64>()
    .ok()
    .filter(|number| number.is_finite())
}

#[cfg(test)]
#[path = "tests/cli_args_test.rs"]
mod tests;
