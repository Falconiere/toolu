//! `Routes`: the replies each exact path serves, in order.

use std::collections::{BTreeMap, VecDeque};

use crate::Reply;

/// Per-path reply sequences. A path serves its replies in order, and its last
/// reply repeats once the others are used.
#[derive(Debug, Default)]
pub(crate) struct Routes(BTreeMap<String, VecDeque<Reply>>);

impl Routes {
  /// Replace `path`'s sequence; an empty sequence removes the route.
  pub(crate) fn set(&mut self, path: &str, replies: Vec<Reply>) {
    if replies.is_empty() {
      self.0.remove(path);
    } else {
      self.0.insert(path.to_owned(), replies.into());
    }
  }

  /// The reply for the next request to `path`.
  pub(crate) fn next(&mut self, path: &str) -> Option<Reply> {
    let replies = self.0.get_mut(path)?;
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
