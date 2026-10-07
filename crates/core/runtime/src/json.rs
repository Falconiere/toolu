//! JSON text as TypeScript prints it: `JSON.stringify` (compact or two-space
//! pretty), JavaScript's `Number#toString` for numbers, and jq's bytes
//! (`toJqJson` in `packages/toolu-core/src/state/state-io.ts`). A
//! `serde_json::Value` prints in `Map` order, which sorts keys where JavaScript
//! keeps insertion order; [`ordered::Ordered`] keeps document order.

pub mod ordered;

use serde_json::Value;

use ordered::Ordered;

/// `JSON.stringify(value)`.
pub fn stringify(value: &Value) -> String {
  Ordered::from(value).to_text(false)
}

/// `JSON.stringify(value, null, 2)`.
pub fn stringify_pretty(value: &Value) -> String {
  Ordered::from(value).to_text(true)
}

/// `value` as `jq` (pretty) or `jq -c` (compact) prints it: `JSON.stringify`
/// with DEL escaped, which only a string can hold.
pub fn jq_text(value: &Ordered, pretty: bool) -> String {
  value.to_text(pretty).replace('\u{7f}', "\\u007f")
}

/// JavaScript's white space (`\s`, what `String#trim` removes): Unicode white
/// space without U+0085, plus the byte-order mark.
pub fn is_js_space(c: char) -> bool {
  (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// JavaScript's `Number#toString` for a finite or non-finite number.
pub fn js_number(number: f64) -> String {
  if number.is_nan() {
    return "NaN".to_owned();
  }
  if number.is_infinite() {
    return if number > 0.0 {
      "Infinity"
    } else {
      "-Infinity"
    }
    .to_owned();
  }
  if number == 0.0 {
    return "0".to_owned();
  }
  let sign = if number < 0.0 { "-" } else { "" };
  let scientific = format!("{:e}", number.abs());
  match scientific.split_once('e') {
    Some((mantissa, exponent)) => match exponent.parse::<i64>() {
      Ok(exponent) => format!("{sign}{}", place(&mantissa.replace('.', ""), exponent + 1)),
      Err(_) => format!("{sign}{scientific}"),
    },
    None => format!("{sign}{scientific}"),
  }
}

/// The digits `digits` with the decimal point after `point` of them, in the
/// layout of ECMA-262 `Number::toString`.
fn place(digits: &str, point: i64) -> String {
  let count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
  let zeros = |n: i64| "0".repeat(usize::try_from(n).unwrap_or(0));
  if count <= point && point <= 21 {
    return format!("{digits}{}", zeros(point - count));
  }
  if 0 < point && point <= 21 {
    let at = usize::try_from(point).unwrap_or(0);
    return match digits.split_at_checked(at) {
      Some((whole, fraction)) => format!("{whole}.{fraction}"),
      None => digits.to_owned(),
    };
  }
  if -6 < point && point <= 0 {
    return format!("0.{}{digits}", zeros(-point));
  }
  let exponent = point - 1;
  let sign = if exponent < 0 { '-' } else { '+' };
  match digits.split_at_checked(1) {
    Some((first, "")) => format!("{first}e{sign}{}", exponent.abs()),
    Some((first, rest)) => format!("{first}.{rest}e{sign}{}", exponent.abs()),
    None => digits.to_owned(),
  }
}

/// The top-level keys of the JSON object `text`, in document order, each once
/// at its first position (`Object.keys(JSON.parse(text))`); `None` when `text`
/// is not a JSON object.
pub fn top_level_keys(text: &str) -> Option<Vec<String>> {
  match Ordered::parse(text).ok()? {
    Ordered::Object(entries) => Some(entries.into_iter().map(|(key, _)| key).collect()),
    Ordered::Null
    | Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::String(_)
    | Ordered::Array(_) => None,
  }
}

#[cfg(test)]
#[path = "tests/json_test.rs"]
mod tests;
