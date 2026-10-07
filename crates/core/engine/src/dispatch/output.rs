//! Hook-output reading and printing (`dispatch-output.ts`, a port of the `jq`
//! calls in `dispatch.sh`). Module stdout is read the way
//! `$(jq -r '<path> // empty' <<<"$result")` reads it, and merged results are
//! printed the way `jq -n` prints them, so the bytes match TypeScript's.

use std::borrow::Cow;

use serde_json::{Map, Value};
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::js_order::js_ordered;

/// `$(...)`: command substitution strips every trailing newline.
pub(crate) fn substituted(text: &str) -> &str {
  text.trim_end_matches('\n')
}

/// `printf '%s\n' "$result"`.
pub(crate) fn printed(result: &str) -> String {
  format!("{result}\n")
}

/// Why a text is not one readable JSON document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unreadable {
  /// Not JSON, as `JSON.parse` would also say.
  NotJson,
  /// JSON nested past serde's recursion limit, which `JSON.parse` would read.
  TooDeep,
}

/// `JSON.parse(text)` with JavaScript key order. Lone surrogate escapes, which
/// `JSON.parse` accepts and serde rejects, read as U+FFFD.
pub(crate) fn parse_document(text: &str) -> Result<Ordered, Unreadable> {
  match Ordered::parse(sanitize_surrogates(text).as_ref()) {
    Ok(value) => Ok(js_ordered(value)),
    Err(message) if message.starts_with("recursion limit exceeded") => Err(Unreadable::TooDeep),
    Err(_) => Err(Unreadable::NotJson),
  }
}

/// The four hex digits after `\u` at `at`, as a code unit.
fn code_unit(bytes: &[u8], at: usize) -> Option<u32> {
  let hex = bytes.get(at..at + 4)?;
  u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()
}

/// `text` with every escape of an unpaired UTF-16 surrogate inside a JSON string
/// replaced by `\ufffd`; a valid pair, other escapes and non-string text are kept.
pub(crate) fn sanitize_surrogates(text: &str) -> Cow<'_, str> {
  if !text.contains("\\u") {
    return Cow::Borrowed(text);
  }
  let bytes = text.as_bytes();
  let mut out = String::with_capacity(text.len());
  let (mut copied, mut at, mut in_string) = (0, 0, false);
  while let Some(&byte) = bytes.get(at) {
    match (byte, in_string) {
      (b'"', _) => in_string = !in_string,
      (b'\\', true) if bytes.get(at + 1) == Some(&b'u') => {
        let unit = code_unit(bytes, at + 2);
        let low = |next: usize| {
          bytes.get(next..next + 2) == Some(b"\\u".as_slice())
            && code_unit(bytes, next + 2).is_some_and(|u| (0xDC00..=0xDFFF).contains(&u))
        };
        match unit {
          Some(0xD800..=0xDBFF) if low(at + 6) => at += 6,
          Some(0xD800..=0xDFFF) => {
            out.push_str(text.get(copied..at).unwrap_or_default());
            out.push_str("\\ufffd");
            copied = at + 6;
          }
          Some(_) | None => {}
        }
        at += 6;
        continue;
      }
      (b'\\', true) => at += 1,
      _ => {}
    }
    at += 1;
  }
  out.push_str(text.get(copied..).unwrap_or_default());
  Cow::Owned(out)
}

/// `value` as a `serde_json::Value`, for readers that need no key order.
pub(crate) fn plain(value: &Ordered) -> Value {
  match value {
    Ordered::Null => Value::Null,
    Ordered::Bool(flag) => Value::Bool(*flag),
    Ordered::Number(number) => Value::Number(number.clone()),
    Ordered::String(text) => Value::String(text.clone()),
    Ordered::Array(items) => Value::Array(items.iter().map(plain).collect()),
    Ordered::Object(entries) => Value::Object(
      entries
        .iter()
        .map(|(key, item)| (key.clone(), plain(item)))
        .collect::<Map<String, Value>>(),
    ),
  }
}

/// `$(jq -r '.<path> // empty' <<<"$result")`: "" when the document is absent,
/// a step indexes a non-object, or the value is null or false.
pub(crate) fn read_field(doc: Option<&Ordered>, path: &[&str]) -> String {
  let mut value = doc;
  for key in path {
    value = value.and_then(|current| match current {
      Ordered::Object(_) => current.get(key),
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Array(_) => None,
    });
  }
  match value {
    None | Some(Ordered::Null | Ordered::Bool(false)) => String::new(),
    Some(Ordered::String(text)) => substituted(text).to_owned(),
    Some(other) => substituted(&jq_text(other, true)).to_owned(),
  }
}

/// Advisories collected over a walk, each deduplicated by exact text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Advisories {
  pub(crate) contexts: Vec<String>,
  pub(crate) messages: Vec<String>,
}

impl Advisories {
  /// Harvest one result's `additionalContext` and `systemMessage`.
  pub(crate) fn collect(&mut self, doc: Option<&Ordered>) {
    add_once(
      &mut self.contexts,
      read_field(doc, &["hookSpecificOutput", "additionalContext"]),
    );
    add_once(&mut self.messages, read_field(doc, &["systemMessage"]));
  }
}

fn add_once(list: &mut Vec<String>, text: String) {
  if !text.is_empty() && !list.contains(&text) {
    list.push(text);
  }
}

/// `jq -n`'s output: two-space pretty print and a newline.
fn jq_print(value: &Ordered) -> String {
  format!("{}\n", jq_text(value, true))
}

/// The truthy value of `value`, or `""` where jq's `// ""` would take the default.
fn or_empty(value: Option<&Ordered>) -> Ordered {
  match value {
    None | Some(Ordered::Null | Ordered::Bool(false)) => Ordered::String(String::new()),
    Some(other) => other.clone(),
  }
}

/// jq's `+` on a string: concatenation, or `None` where jq would error.
fn concat(left: &Ordered, right: &str) -> Option<Ordered> {
  match left {
    Ordered::String(text) => Some(Ordered::String(format!("{text}{right}"))),
    Ordered::Null
    | Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::Array(_)
    | Ordered::Object(_) => None,
  }
}

fn enriched(ask: &Ordered, ctx: &str, msg: &str) -> Option<Ordered> {
  let hso = ask
    .get("hookSpecificOutput")
    .filter(|hso| matches!(hso, Ordered::Object(_)))?;
  let reason = or_empty(hso.get("permissionDecisionReason"));
  let next_reason = if ctx.is_empty() {
    reason
  } else {
    concat(&reason, &format!("\n\n{ctx}"))?
  };
  let mut hso = hso.clone();
  hso.set("permissionDecisionReason", next_reason);
  let mut out = ask.clone();
  out.set("hookSpecificOutput", hso);
  if msg.is_empty() {
    return Some(out);
  }
  let base = or_empty(ask.get("systemMessage"));
  let next_msg = if base == Ordered::String(String::new()) {
    Ordered::String(msg.to_owned())
  } else {
    concat(&base, &format!("\n\n{msg}"))?
  };
  out.set("systemMessage", next_msg);
  Some(out)
}

/// The held ask with every advisory appended to its reason (contexts) and its
/// `systemMessage` (messages); where jq's enrichment would fail, the ask as written.
pub(crate) fn final_ask(ask: &str, advisories: &Advisories) -> String {
  let doc = parse_document(ask).ok();
  let out = doc
    .as_ref()
    .filter(|doc| matches!(doc, Ordered::Object(_)))
    .and_then(|doc| {
      enriched(
        doc,
        &advisories.contexts.join("\n\n"),
        &advisories.messages.join("\n\n"),
      )
    });
  out.map_or_else(|| printed(ask), |out| jq_print(&out))
}

/// The merged advisory object for `event_name`, or "" when there is nothing to say.
pub(crate) fn final_advisory(advisories: &Advisories, event_name: &str) -> String {
  let ctx = advisories.contexts.join("\n\n");
  let msg = advisories.messages.join("\n\n");
  let mut out = Vec::new();
  if !ctx.is_empty() {
    let hso = vec![
      (
        "hookEventName".to_owned(),
        Ordered::String(event_name.to_owned()),
      ),
      ("additionalContext".to_owned(), Ordered::String(ctx)),
    ];
    out.push(("hookSpecificOutput".to_owned(), Ordered::Object(hso)));
  }
  if !msg.is_empty() {
    out.push(("systemMessage".to_owned(), Ordered::String(msg)));
  }
  if out.is_empty() {
    return String::new();
  }
  jq_print(&Ordered::Object(out))
}

#[cfg(test)]
#[path = "tests/output_test.rs"]
mod tests;
