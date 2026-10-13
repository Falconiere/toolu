//! `OpenCode`'s JSONC config files: read them failing closed, and edit them
//! keeping comments and formatting outside the edited span. The edits the
//! TypeScript CLI made with `jsonc-parser`'s `modify` go through this crate's
//! CST; appending on a line of its own and removing a line are in `splice`.

use std::path::{Path, PathBuf};

use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::{CollectOptions, ParseOptions, parse_to_ast};
use serde_json::{Map, Value};

use crate::fail::{Fail, Res};

/// One config file; an absent file has neither text nor data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigFile {
  /// Where it lives.
  pub(crate) path: PathBuf,
  /// Its text.
  pub(crate) text: Option<String>,
  /// Its object.
  pub(crate) data: Option<Map<String, Value>>,
}

/// JSON plus comments and trailing commas, as `OpenCode` reads it.
pub(crate) fn strict() -> ParseOptions {
  ParseOptions {
    allow_comments: true,
    allow_loose_object_property_names: false,
    allow_trailing_commas: true,
    allow_missing_commas: false,
    allow_single_quoted_strings: false,
    allow_hexadecimal_numbers: false,
    allow_unary_plus_numbers: false,
    allow_bare_decimal_point_numbers: false,
    allow_non_finite_numbers: false,
    allow_extended_string_escapes: false,
  }
}

/// The object `text` holds; anything else names the file and writes nothing.
pub(crate) fn parse_config(path: &Path, text: &str) -> Res<Map<String, Value>> {
  let label = path.display();
  let parsed = parse_to_ast(text, &CollectOptions::default(), &strict()).map_err(|err| {
    Fail::failed(format!(
      "{label} is not valid JSONC ({} at offset {}); nothing was written",
      err.kind(),
      err.range().start
    ))
  })?;
  match parsed.value.map(Value::from) {
    Some(Value::Object(map)) => Ok(map),
    _other => Err(Fail::failed(format!(
      "{label} must hold a JSON object; nothing was written"
    ))),
  }
}

/// Reads `path`: absent is absent, unreadable or invalid fails.
pub(crate) fn read_config_file(path: &Path) -> Res<ConfigFile> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      return Ok(ConfigFile {
        path: path.to_path_buf(),
        text: None,
        data: None,
      });
    }
    Err(err) => return Err(Fail::failed(format!("cannot read {}: {err}", path.display()))),
  };
  let data = parse_config(path, &text)?;
  Ok(ConfigFile {
    path: path.to_path_buf(),
    text: Some(text),
    data: Some(data),
  })
}

/// A new config holding only the schema and `spec`.
pub(crate) fn new_config_text(spec: &str) -> String {
  let doc = serde_json::json!({ "$schema": "https://opencode.ai/config.json", "plugin": [spec] });
  let pretty = serde_json::to_string_pretty(&doc).unwrap_or_default();
  format!("{pretty}\n")
}

fn edit(text: &str, change: impl FnOnce(&CstRootNode)) -> String {
  match CstRootNode::parse(text, &strict()) {
    Ok(root) => {
      change(&root);
      root.to_string()
    }
    Err(_unparsable) => text.to_owned(),
  }
}

/// Adds `"plugin": [spec]` to the root object.
pub(crate) fn add_plugin_key(text: &str, spec: &str) -> String {
  edit(text, |root| {
    let entry = CstInputValue::Array(vec![CstInputValue::String(spec.to_owned())]);
    root.object_value_or_set().append("plugin", entry);
  })
}

/// Removes the root object's `key`.
pub(crate) fn remove_key(text: &str, key: &str) -> String {
  edit(text, |root| {
    if let Some(prop) = root.object_value().and_then(|object| object.get(key)) {
      prop.remove();
    }
  })
}

/// Appends `spec` to the root's `key` array where `splice` cannot keep a line.
pub(crate) fn append_element(text: &str, key: &str, spec: &str) -> String {
  edit(text, |root| {
    let array = root.object_value_or_set().array_value_or_set(key);
    array.append(CstInputValue::String(spec.to_owned()));
  })
}

/// Removes element `index` of the root's `key` array.
pub(crate) fn remove_element(text: &str, key: &str, index: usize) -> String {
  edit(text, |root| {
    let array = root.object_value().and_then(|object| object.array_value(key));
    if let Some(element) = array.and_then(|array| array.elements().into_iter().nth(index)) {
      element.remove();
    }
  })
}

/// Replaces the spec string of `plugin[index]` (or of `plugin[index][0]`, a
/// tuple) with `spec`, leaving every other byte alone.
pub(crate) fn replace_spec(text: &str, index: usize, tuple: bool, spec: &str) -> String {
  let Ok(parsed) = parse_to_ast(text, &CollectOptions::default(), &strict()) else {
    return text.to_owned();
  };
  let array = parsed
    .value
    .as_ref()
    .and_then(|value| value.as_object())
    .and_then(|object| object.get("plugin"))
    .and_then(|prop| prop.value.as_array());
  let element = array.and_then(|array| array.elements.get(index));
  let target = if tuple {
    element.and_then(|value| value.as_array()).and_then(|tuple| tuple.elements.first())
  } else {
    element
  };
  let Some(range) = target.and_then(|value| value.as_string_lit()).map(|lit| lit.range) else {
    return text.to_owned();
  };
  let quoted = serde_json::to_string(spec).unwrap_or_default();
  let head = text.get(..range.start).unwrap_or_default();
  let tail = text.get(range.end..).unwrap_or_default();
  format!("{head}{quoted}{tail}")
}

#[cfg(test)]
#[path = "tests/jsonc_test.rs"]
mod tests;
