//! `Routes`: the replies each exact path serves, in order, optionally only
//! to requests whose body contains a needle.

use std::collections::{BTreeMap, VecDeque};

use crate::Reply;

/// Per-path reply sequences. A path serves its replies in order, and its last
/// reply repeats once the others are used. A sequence with a non-empty needle
/// answers only requests whose body contains it, before the path's plain one.
#[derive(Debug, Default)]
pub(crate) struct Routes(BTreeMap<(String, String), VecDeque<Reply>>);

impl Routes {
  /// Replace `path`'s sequence; an empty sequence removes the route.
  pub(crate) fn set(&mut self, path: &str, replies: Vec<Reply>) {
    self.set_when(path, "", replies);
  }

  /// Replace `path`'s sequence for bodies containing `needle`.
  pub(crate) fn set_when(&mut self, path: &str, needle: &str, replies: Vec<Reply>) {
    let key = (path.to_owned(), needle.to_owned());
    if replies.is_empty() {
      self.0.remove(&key);
    } else {
      self.0.insert(key, replies.into());
    }
  }

  /// The reply for the next request to `path` with `body`.
  pub(crate) fn next(&mut self, path: &str, body: &[u8]) -> Option<Reply> {
    let text = String::from_utf8_lossy(body);
    let key = self
      .0
      .keys()
      .find(|(at, needle)| at == path && !needle.is_empty() && text.contains(needle.as_str()))
      .cloned()
      .unwrap_or_else(|| (path.to_owned(), String::new()));
    let replies = self.0.get_mut(&key)?;
    if replies.len() > 1 {
      replies.pop_front()
    } else {
      replies.front().cloned()
    }
  }
}

#[cfg(test)]
#[path = "tests/routes_test.rs"]
mod tests;
