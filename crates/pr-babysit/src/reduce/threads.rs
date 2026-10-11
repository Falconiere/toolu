//! Review threads, conversation comments and reviews, classified into what the
//! agent must answer, what an active fixer owns, and what the resolution audit
//! still counts.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value, json};

use crate::json::{array, has_key, jq_string, nil, or, text};

const CI_REVIEWERS: [&str; 4] = [
  "github-actions",
  "github-actions[bot]",
  "claude",
  "claude[bot]",
];

/// Whether `login` is one of the CI reviewers, compared exactly.
pub(crate) fn is_ci_reviewer(login: &str) -> bool {
  CI_REVIEWERS.contains(&login)
}

const INJECTION: [&str; 6] = [
  "ignore (all |any |the )?(previous|prior|above|earlier) (instructions|prompts?|rules)",
  "disregard (all |any |the |your )?(previous|prior|system|above) ",
  "you are (now )?(an? )?(ai|assistant|llm|language model|claude|codex|copilot)",
  "(^|\\n)\\s*(system|assistant)\\s*:",
  "<(system|instructions?)>",
  "(run|execute) (the following|this|these) (command|shell|script)",
];

static INJECTION_RE: LazyLock<Vec<(&'static str, Option<Regex>)>> = LazyLock::new(|| {
  INJECTION
    .iter()
    .map(|source| (*source, Regex::new(&format!("(?i){source}")).ok()))
    .collect()
});

/// The thread sets of one tick.
#[derive(Debug, Default)]
pub(crate) struct Threads {
  /// Threads needing a new disposition, not owned by a fixer.
  pub(crate) actionable: Vec<Value>,
  /// Actionable threads an active fixer owns.
  pub(crate) fixing: Vec<Value>,
  /// Audited threads the author replied to without a resolve.
  pub(crate) stale: Vec<Value>,
  /// Outdated CI-reviewer thread ids.
  pub(crate) skipped: Vec<Value>,
  /// Thread ids flagged as injection.
  pub(crate) flagged: Vec<Value>,
  /// The resolution-audit count.
  pub(crate) unresolved: usize,
}

struct Flags {
  actionable: bool,
  audited: bool,
  skipped: bool,
  /// The thread id when the thread is flagged as injection.
  flagged: Option<Value>,
}

fn author_class(comment: Option<&Value>) -> &'static str {
  let Some(comment) = comment else {
    return "none";
  };
  if is_ci_reviewer(text(comment, "author")) {
    "ci_reviewer"
  } else if text(comment, "authorType") == "Bot" {
    "bot"
  } else {
    "human"
  }
}

fn injection(non_author: &[&Value]) -> Option<&'static str> {
  INJECTION_RE.iter().find_map(|(source, re)| {
    let re = re.as_ref()?;
    non_author
      .iter()
      .any(|comment| {
        re.is_match(
          nil(comment.get("body"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        )
      })
      .then_some(*source)
  })
}

fn classify(thread: &Value, author: &Value, actions: &Value) -> (Value, Flags) {
  let comments = array(thread.get("comments"));
  let last = comments.last();
  let non_author: Vec<&Value> = comments
    .iter()
    .filter(|comment| comment.get("author") != Some(author))
    .collect();
  let last_other = non_author.last().copied();
  let id = thread.get("id").cloned().unwrap_or(Value::Null);
  let flagged = has_key(actions.get("flagged"), &jq_string(&id));
  let class = author_class(last_other);
  let outdated = nil(thread.get("isOutdated")).is_some();
  let open = thread.get("isResolved") == Some(&Value::Bool(false));
  let answerable = open
    && !flagged
    && last.is_some_and(|comment| comment.get("author") != Some(author))
    && matches!(class, "human" | "ci_reviewer");
  let flags = Flags {
    actionable: if outdated {
      answerable && class == "human"
    } else {
      answerable
    },
    audited: open && !flagged && (!outdated || class == "human"),
    skipped: open && outdated && class == "ci_reviewer",
    flagged: flagged.then(|| id.clone()),
  };
  let pattern = injection(&non_author);
  let item = json!({
    "id": id,
    "path": thread.get("path").cloned().unwrap_or(Value::Null),
    "line": thread.get("line").cloned().unwrap_or(Value::Null),
    "isOutdated": thread.get("isOutdated").cloned().unwrap_or(Value::Null),
    "rootCommentId": or(comments.first().and_then(|c| c.get("databaseId")), Value::Null),
    "inReplyTo": or(last_other.and_then(|c| c.get("databaseId")), Value::Null),
    "authorClass": class,
    "lastCommentAuthor": or(last.and_then(|c| c.get("author")), Value::Null),
    "lastCommentAt": or(last.and_then(|c| c.get("createdAt")), Value::Null),
    "injectionSuspect": pattern.is_some(),
    "injectionPattern": pattern,
    "comments": comments,
  });
  (item, flags)
}

/// Classify every review thread of the snapshot.
pub(crate) fn threads(
  snapshot: &Value,
  author: &Value,
  actions: &Value,
  owned: &dyn Fn(&Value) -> bool,
) -> Threads {
  let mut sets = Threads::default();
  for thread in array(snapshot.get("threads")) {
    let (item, flags) = classify(thread, author, actions);
    let get = |key: &str| item.get(key).cloned().unwrap_or(Value::Null);
    if flags.audited {
      sets.unresolved += 1;
      if !flags.actionable {
        sets.stale.push(json!({
          "id": get("id"), "path": get("path"), "line": get("line"),
          "repliedAt": get("lastCommentAt"), "lastCommentAuthor": get("lastCommentAuthor"),
        }));
      }
    }
    if flags.skipped {
      sets.skipped.push(get("id"));
    }
    sets.flagged.extend(flags.flagged);
    if flags.actionable {
      if owned(&item) {
        sets.fixing.push(item);
      } else {
        sets.actionable.push(item);
      }
    }
  }
  sets
}

fn pick(item: &Value, keys: &[&str]) -> Value {
  let mut picked = Map::new();
  for key in keys {
    if let Some(value) = item.get(*key) {
      picked.insert((*key).to_owned(), value.clone());
    }
  }
  Value::Object(picked)
}

fn split(items: Vec<Value>, owned: &dyn Fn(&Value) -> bool) -> (Vec<Value>, Vec<Value>) {
  items.into_iter().partition(|item| !owned(item))
}

/// Human conversation comments with no later author comment and no reply:
/// `(actionable, fixing)`.
pub(crate) fn conversation(
  snapshot: &Value,
  author: &Value,
  replied: Option<&Value>,
  owned: &dyn Fn(&Value) -> bool,
) -> (Vec<Value>, Vec<Value>) {
  let comments = array(snapshot.get("comments"));
  let answered_later = |comment: &Value| {
    comments.iter().any(|later| {
      later.get("author") == Some(author)
        && matches!(
          (later.get("createdAt").and_then(Value::as_str), comment.get("createdAt").and_then(Value::as_str)),
          (Some(after), Some(before)) if after > before
        )
    })
  };
  let items = comments
    .iter()
    .filter(|comment| comment.get("author") != Some(author) && text(comment, "authorType") != "Bot")
    .filter(|comment| !answered_later(comment))
    .filter(|comment| {
      let id = comment
        .get("id")
        .map_or_else(|| "null".to_owned(), jq_string);
      !has_key(replied, &format!("conversation:{id}"))
    })
    .map(|comment| pick(comment, &["id", "author", "body", "createdAt", "url"]))
    .collect();
  split(items, owned)
}

/// Human non-approving reviews with a body and no reply: `(actionable, fixing)`.
pub(crate) fn reviews(
  snapshot: &Value,
  author: &Value,
  replied: Option<&Value>,
  owned: &dyn Fn(&Value) -> bool,
) -> (Vec<Value>, Vec<Value>) {
  let items = array(snapshot.get("reviews"))
    .iter()
    .filter(|review| {
      review.get("author") != Some(author)
        && text(review, "authorType") != "Bot"
        && text(review, "state") != "APPROVED"
        && nil(review.get("body"))
          .and_then(Value::as_str)
          .is_some_and(|body| !body.is_empty())
    })
    .filter(|review| {
      let id = review
        .get("id")
        .map_or_else(|| "null".to_owned(), jq_string);
      !has_key(replied, &format!("review:{id}"))
    })
    .map(|review| {
      pick(
        review,
        &["id", "author", "state", "body", "submittedAt", "url"],
      )
    })
    .collect();
  split(items, owned)
}

#[cfg(test)]
#[path = "tests/threads_test.rs"]
mod tests;
