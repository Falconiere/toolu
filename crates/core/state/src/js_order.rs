//! JavaScript object key order: array-index keys (canonical decimals below
//! 2³² − 1) first, ascending, then every other key in insertion order. The
//! TypeScript writers hold gate entries in such objects, so a byte-identical
//! file keeps their order, and a parsed hook payload reads the way
//! TypeScript's dispatcher reads it.

use toolu_runtime::json::ordered::Ordered;

/// Whether `key` is a canonical array index.
pub(crate) fn is_array_index(key: &str) -> bool {
  key
    .parse::<u32>()
    .is_ok_and(|index| index != u32::MAX && index.to_string() == key)
}

/// `entries` reordered as a JavaScript object orders its keys.
pub(crate) fn js_order<T>(entries: Vec<(String, T)>) -> Vec<(String, T)> {
  let (mut indices, rest): (Vec<_>, Vec<_>) = entries
    .into_iter()
    .partition(|(key, _)| is_array_index(key));
  indices.sort_by_key(|(key, _)| key.parse::<u32>().unwrap_or(u32::MAX));
  indices.extend(rest);
  indices
}

/// `value` with every object, at every depth, in `JSON.parse`'s key order.
pub fn js_ordered(value: Ordered) -> Ordered {
  match value {
    Ordered::Object(entries) => Ordered::Object(js_order(
      entries
        .into_iter()
        .map(|(key, item)| (key, js_ordered(item)))
        .collect(),
    )),
    Ordered::Array(items) => Ordered::Array(items.into_iter().map(js_ordered).collect()),
    scalar @ (Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_)) => scalar,
  }
}

#[cfg(test)]
#[path = "tests/js_order_test.rs"]
mod tests;
