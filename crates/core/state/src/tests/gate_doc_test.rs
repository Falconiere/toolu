use toolu_runtime::json::ordered::Ordered;

use super::{
  Failure, cleared_doc, doc_text, dropped_count, failing_doc, seed_entries, single_slot,
};
use crate::gate_schema::{GateEntry, GateFile};

fn entry(source: &str, at: &str, violations: &str) -> GateEntry {
  GateEntry {
    source: source.into(),
    reason: format!("reason of {source}"),
    violations: violations.into(),
    updated_at: at.into(),
  }
}

fn failure<'a>(file: &'a str, now: &'a str) -> Failure<'a> {
  Failure {
    file,
    source: "s",
    reason: "r",
    violations: "new\n",
    now,
  }
}

fn keys(doc: &GateFile) -> Vec<String> {
  let GateFile::Failing {
    entries: Some(entries),
    ..
  } = doc
  else {
    panic!("no entries")
  };
  entries.iter().map(|(key, _)| key.clone()).collect()
}

#[test]
fn the_seed_is_the_entries_or_the_legacy_single_slot() {
  let passing = GateFile::Passing {
    source: "s".into(),
    updated_at: "u".into(),
  };
  assert_eq!(seed_entries(&passing), Vec::new());
  let legacy = GateFile::Failing {
    reason: "r".into(),
    source: "s".into(),
    file: "/a".into(),
    violations: "v".into(),
    entries: None,
    updated_at: "u".into(),
  };
  let seed = seed_entries(&legacy);
  assert_eq!(seed.len(), 1);
  assert_eq!(
    (seed[0].0.as_str(), seed[0].1.violations.as_str()),
    ("/a", "v")
  );
}

#[test]
fn a_record_replaces_in_place_appends_and_joins_oldest_first() {
  let prev = vec![
    ("/b".to_owned(), entry("x", "2020-01-02T00:00:00Z", "b\n")),
    ("/a".to_owned(), entry("y", "2020-01-01T00:00:00Z", "a\n")),
  ];
  let doc = failing_doc(prev.clone(), &failure("/b", "2026-01-01T00:00:00Z"));
  assert_eq!(keys(&doc), ["/b", "/a"], "replaced in place");
  let GateFile::Failing {
    violations, file, ..
  } = &doc
  else {
    panic!()
  };
  assert_eq!((violations.as_str(), file.as_str()), ("a\nnew\n", "/b"));
  let doc = failing_doc(prev, &failure("10", "2026-01-01T00:00:00Z"));
  assert_eq!(
    keys(&doc),
    ["10", "/b", "/a"],
    "an integer-like key goes first, as in JavaScript"
  );
}

#[test]
fn equal_timestamps_order_by_key_bytes() {
  let same = "2020-01-01T00:00:00Z";
  let prev = vec![
    ("é".to_owned(), entry("x", same, "e\n")),
    ("z".to_owned(), entry("x", same, "z\n")),
  ];
  let GateFile::Failing { violations, .. } = failing_doc(prev, &failure("a", same)) else {
    panic!()
  };
  assert_eq!(violations, "new\nz\ne\n");
}

#[test]
fn a_clear_promotes_the_latest_slot_or_passes() {
  let left = vec![
    ("/old".to_owned(), entry("x", "2020-01-01T00:00:00Z", "o\n")),
    ("/new".to_owned(), entry("y", "2020-01-02T00:00:00Z", "n\n")),
  ];
  let GateFile::Failing {
    file,
    source,
    reason,
    violations,
    updated_at,
    entries,
  } = cleared_doc(left, "s", "now")
  else {
    panic!("passing");
  };
  assert_eq!(
    (file.as_str(), source.as_str(), reason.as_str()),
    ("/new", "y", "reason of y")
  );
  assert_eq!(
    (violations.as_str(), updated_at.as_str()),
    ("o\nn\n", "now")
  );
  assert_eq!(entries.unwrap().len(), 2);
  assert_eq!(
    cleared_doc(Vec::new(), "s", "now"),
    GateFile::Passing {
      source: "s".into(),
      updated_at: "now".into()
    }
  );
}

#[test]
fn dropped_counts_follow_the_seed_rule() {
  let json = |text: &str| Ordered::parse(text).unwrap();
  assert_eq!(
    dropped_count(&json(r#"{"entries":{"/a":1,"/b":2},"x":1}"#), "/a"),
    1
  );
  assert_eq!(
    dropped_count(&json(r#"{"status":"failing","file":"/a"}"#), "/a"),
    0
  );
  assert_eq!(
    dropped_count(&json(r#"{"status":"failing","file":"/a"}"#), "/c"),
    1
  );
  assert_eq!(
    dropped_count(&json(r#"{"status":"failing","file":7}"#), "/c"),
    1,
    "__global__"
  );
  assert_eq!(dropped_count(&json(r#"{"status":"passing"}"#), "/c"), 0);
  assert_eq!(dropped_count(&json("[1,2]"), "/c"), 0);
}

#[test]
fn texts_are_two_space_jq_with_a_newline() {
  let text = doc_text(&single_slot(&failure("/a", "t")));
  let expected = "{\n  \"status\": \"failing\",\n  \"reason\": \"r\",\n  \"source\": \"s\",\n  \"file\": \"/a\",\n  \"violations\": \"new\\n\",\n  \"updatedAt\": \"t\"\n}\n";
  assert_eq!(text, expected);
}
