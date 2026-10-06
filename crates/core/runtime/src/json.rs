//! JSON text as TypeScript prints it: `JSON.stringify` (compact or two-space
//! pretty), JavaScript's `Number#toString` for numbers, and jq's bytes
//! (`toJqJson` in `packages/toolu-core/src/state/state-io.ts`). Objects print
//! in `serde_json::Map` order, which sorts keys where JavaScript keeps
//! insertion order. Also the top-level keys of a document in document order,
//! which a `Map` loses.

use serde::Deserializer as _;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde_json::Value;

/// `JSON.stringify(value)`.
pub fn stringify(value: &Value) -> String {
  let mut out = String::new();
  write_value(&mut out, value, None, 0);
  out
}

/// `JSON.stringify(value, null, 2)`.
pub fn stringify_pretty(value: &Value) -> String {
  let mut out = String::new();
  write_value(&mut out, value, Some(2), 0);
  out
}

/// `value` as `jq` (pretty) or `jq -c` (compact) prints it: `JSON.stringify`
/// with DEL escaped, which only a string can hold.
pub fn jq_text(value: &Value, pretty: bool) -> String {
  let text = if pretty {
    stringify_pretty(value)
  } else {
    stringify(value)
  };
  text.replace('\u{7f}', "\\u007f")
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

fn write_value(out: &mut String, value: &Value, indent: Option<usize>, depth: usize) {
  match value {
    Value::Null => out.push_str("null"),
    Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
    Value::Number(number) => out.push_str(
      &number
        .as_f64()
        .map_or_else(|| number.to_string(), js_number),
    ),
    Value::String(text) => out.push_str(&Value::from(text.as_str()).to_string()),
    Value::Array(items) => {
      let items = items.iter().map(|item| (None, item));
      write_block(out, ('[', ']'), items, indent, depth);
    }
    Value::Object(map) => {
      let entries = map.iter().map(|(key, item)| (Some(key.as_str()), item));
      write_block(out, ('{', '}'), entries, indent, depth);
    }
  }
}

fn write_block<'a>(
  out: &mut String,
  (open, close): (char, char),
  entries: impl Iterator<Item = (Option<&'a str>, &'a Value)>,
  indent: Option<usize>,
  depth: usize,
) {
  out.push(open);
  let mut empty = true;
  for (key, item) in entries {
    if !empty {
      out.push(',');
    }
    empty = false;
    newline(out, indent, depth + 1);
    if let Some(key) = key {
      out.push_str(&Value::from(key).to_string());
      out.push_str(if indent.is_some() { ": " } else { ":" });
    }
    write_value(out, item, indent, depth + 1);
  }
  if !empty {
    newline(out, indent, depth);
  }
  out.push(close);
}

fn newline(out: &mut String, indent: Option<usize>, depth: usize) {
  if let Some(width) = indent {
    out.push('\n');
    out.push_str(&" ".repeat(width * depth));
  }
}

/// The top-level keys of the JSON object `text`, in document order, each once
/// at its first position (`Object.keys(JSON.parse(text))`); `None` when `text`
/// is not a JSON object.
pub fn top_level_keys(text: &str) -> Option<Vec<String>> {
  let mut deserializer = serde_json::Deserializer::from_str(text);
  deserializer.deserialize_any(KeyOrder).ok()
}

/// Collects an object's keys and skips its values.
struct KeyOrder;

impl<'de> Visitor<'de> for KeyOrder {
  type Value = Vec<String>;

  fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("a JSON object")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Vec<String>, A::Error> {
    let mut keys: Vec<String> = Vec::new();
    while let Some(key) = map.next_key::<String>()? {
      map.next_value::<IgnoredAny>()?;
      if !keys.contains(&key) {
        keys.push(key);
      }
    }
    Ok(keys)
  }
}

#[cfg(test)]
#[path = "tests/json_test.rs"]
mod tests;
