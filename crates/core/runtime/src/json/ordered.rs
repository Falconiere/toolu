//! `Ordered`: a JSON value whose objects keep document order, as a JavaScript
//! object does, and its `JSON.stringify` text. `serde_json::Map` sorts keys, so
//! a file rewritten through it would come back reordered.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Number, Value};

use super::js_number;

/// A JSON value with insertion-ordered objects.
#[derive(Debug, Clone, PartialEq)]
pub enum Ordered {
  /// `null`.
  Null,
  /// `true` or `false`.
  Bool(bool),
  /// A number.
  Number(Number),
  /// A string.
  String(String),
  /// An array.
  Array(Vec<Ordered>),
  /// An object; a repeated key keeps its first position and its last value.
  Object(Vec<(String, Ordered)>),
}

impl Ordered {
  /// `JSON.parse(text)`.
  ///
  /// # Errors
  /// The parser's message when `text` is not one JSON document.
  pub fn parse(text: &str) -> Result<Ordered, String> {
    serde_json::from_str(text).map_err(|err| err.to_string())
  }

  /// `self[key]` of an object.
  pub fn get(&self, key: &str) -> Option<&Ordered> {
    match self {
      Ordered::Object(entries) => entries
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value),
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Array(_) => None,
    }
  }

  /// Sets `key` of an object in place, or appends it; anything else is left alone.
  pub fn set(&mut self, key: &str, value: Ordered) {
    if let Ordered::Object(entries) = self {
      match entries.iter_mut().find(|(name, _)| name == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key.to_owned(), value)),
      }
    }
  }

  /// `JSON.stringify(self)`, or `JSON.stringify(self, null, 2)` when `pretty`.
  pub fn to_text(&self, pretty: bool) -> String {
    let mut out = String::new();
    self.write(&mut out, pretty.then_some(2), 0);
    out
  }

  fn write(&self, out: &mut String, indent: Option<usize>, depth: usize) {
    match self {
      Ordered::Null => out.push_str("null"),
      Ordered::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
      Ordered::Number(number) => out.push_str(
        &number
          .as_f64()
          .map_or_else(|| number.to_string(), js_number),
      ),
      Ordered::String(text) => out.push_str(&Value::from(text.as_str()).to_string()),
      Ordered::Array(items) => write_block(
        out,
        ('[', ']'),
        items.iter().map(|item| (None, item)),
        indent,
        depth,
      ),
      Ordered::Object(entries) => {
        let entries = entries.iter().map(|(key, item)| (Some(key.as_str()), item));
        write_block(out, ('{', '}'), entries, indent, depth);
      }
    }
  }
}

fn write_block<'a>(
  out: &mut String,
  (open, close): (char, char),
  entries: impl Iterator<Item = (Option<&'a str>, &'a Ordered)>,
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
    item.write(out, indent, depth + 1);
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

impl From<&Value> for Ordered {
  /// `value`, its objects in `serde_json::Map` order.
  fn from(value: &Value) -> Ordered {
    match value {
      Value::Null => Ordered::Null,
      Value::Bool(flag) => Ordered::Bool(*flag),
      Value::Number(number) => Ordered::Number(number.clone()),
      Value::String(text) => Ordered::String(text.clone()),
      Value::Array(items) => Ordered::Array(items.iter().map(Ordered::from).collect()),
      Value::Object(map) => Ordered::Object(
        map
          .iter()
          .map(|(key, item)| (key.clone(), Ordered::from(item)))
          .collect(),
      ),
    }
  }
}

impl<'de> Deserialize<'de> for Ordered {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Ordered, D::Error> {
    deserializer.deserialize_any(OrderedVisitor)
  }
}

/// Builds an [`Ordered`] from any JSON token.
struct OrderedVisitor;

impl<'de> Visitor<'de> for OrderedVisitor {
  type Value = Ordered;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a JSON value")
  }

  fn visit_unit<E>(self) -> Result<Ordered, E> {
    Ok(Ordered::Null)
  }

  fn visit_bool<E>(self, flag: bool) -> Result<Ordered, E> {
    Ok(Ordered::Bool(flag))
  }

  fn visit_i64<E>(self, number: i64) -> Result<Ordered, E> {
    Ok(Ordered::Number(number.into()))
  }

  fn visit_u64<E>(self, number: u64) -> Result<Ordered, E> {
    Ok(Ordered::Number(number.into()))
  }

  fn visit_f64<E: serde::de::Error>(self, number: f64) -> Result<Ordered, E> {
    Number::from_f64(number)
      .map(Ordered::Number)
      .ok_or_else(|| E::custom("a non-finite number"))
  }

  fn visit_str<E>(self, text: &str) -> Result<Ordered, E> {
    Ok(Ordered::String(text.to_owned()))
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ordered, A::Error> {
    let mut items = Vec::new();
    while let Some(item) = seq.next_element()? {
      items.push(item);
    }
    Ok(Ordered::Array(items))
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
    let mut object = Ordered::Object(Vec::new());
    while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
      object.set(&key, value);
    }
    Ok(object)
  }
}

#[cfg(test)]
#[path = "tests/ordered_test.rs"]
mod tests;
