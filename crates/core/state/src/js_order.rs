//! JavaScript object key order: array-index keys (canonical decimals below
//! 2³² − 1) first, ascending, then every other key in insertion order. The
//! TypeScript writers hold gate entries in such objects, so a byte-identical
//! file keeps their order.

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

#[cfg(test)]
#[path = "tests/js_order_test.rs"]
mod tests;
