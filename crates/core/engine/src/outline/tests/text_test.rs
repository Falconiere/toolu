use super::{cap, item};
use crate::outline::parse::Match;
use crate::outline::rules::Language;

fn at(
  lines: &[&str],
  start: usize,
  end: usize,
  name: &str,
  language: Language,
) -> crate::outline::Item {
  let found = Match {
    file: "x".to_owned(),
    start,
    end,
    name: name.to_owned(),
  };
  item(lines, &found, language)
}

#[test]
fn outline_text_extends_over_a_multi_line_attribute() {
  let lines = [
    "",
    "/// Run it.",
    "#[cfg(any(",
    "  unix,",
    "))]",
    "pub(crate) fn run() {}",
  ];
  let found = at(&lines, 5, 5, "run", Language::Rust);
  assert_eq!((found.start, found.end), (2, 6));
  assert_eq!(found.signature, "pub(crate) fn run()");
  assert_eq!(found.doc.as_deref(), Some("Run it."));
  assert!(found.public);
}

#[test]
fn outline_text_stops_at_a_plain_comment_and_reads_one_line_jsdoc() {
  let lines = [
    "/* not a doc */",
    "function a(): void {}",
    "/** Says hi. */",
    "export const b = 1;",
  ];
  let plain = at(&lines, 1, 1, "a", Language::Script);
  assert_eq!((plain.start, plain.doc, plain.public), (2, None, false));
  let documented = at(&lines, 3, 3, "b", Language::Script);
  assert_eq!(
    (documented.start, documented.doc.as_deref()),
    (3, Some("Says hi."))
  );
  assert_eq!(documented.signature, "export const b = 1");
}

#[test]
fn outline_text_drops_trailing_comments_and_keeps_python_private_names() {
  let lines = ["def _hidden(a,  # first", "            b):", "    return a"];
  let found = at(&lines, 0, 2, "_hidden", Language::Python);
  assert_eq!(found.signature, "def _hidden(a, b)");
  assert_eq!(found.doc, None);
  assert!(!found.public);
}

#[test]
fn outline_text_caps_long_signatures_and_docs() {
  let long = format!("pub fn f({}) {{}}", "a: u8, ".repeat(40));
  let doc = format!("/// {}", "word ".repeat(40));
  let lines = [doc.as_str(), long.as_str()];
  let found = at(&lines, 1, 1, "f", Language::Rust);
  assert_eq!(found.signature.chars().count(), 160);
  assert!(found.signature.ends_with("..."));
  assert_eq!(found.doc.map(|doc| doc.chars().count()), Some(100));
  assert_eq!(cap("short", 10), "short");
}

#[test]
fn outline_text_ignores_openers_inside_string_literals() {
  let lines = [
    "const NOTE: &str = \"a; b {c}\";",
    "export const s = 'x;y';",
  ];
  assert_eq!(
    at(&lines, 0, 0, "NOTE", Language::Rust).signature,
    "const NOTE: &str = \"a; b {c}\""
  );
  assert_eq!(
    at(&lines, 1, 1, "s", Language::Script).signature,
    "export const s = 'x;y'"
  );
}

#[test]
fn outline_text_does_not_climb_past_a_call_into_an_earlier_decorator() {
  let lines = [
    "@dec",
    "def a(): pass",
    "x = foo(1)",
    "def b():",
    "    pass",
  ];
  assert_eq!(at(&lines, 3, 4, "b", Language::Python).start, 4);
  let multi = ["@route(", "    \"/x\",", ")", "def c():", "    pass"];
  assert_eq!(at(&multi, 3, 4, "c", Language::Python).start, 1);
}

#[test]
fn outline_text_reads_the_docstring_of_an_indented_method() {
  let lines = [
    "class Shape:",
    "    def area(self):",
    "        \"\"\"Area of it.\"\"\"",
    "        return 0",
  ];
  assert_eq!(
    at(&lines, 1, 3, "area", Language::Python).doc.as_deref(),
    Some("Area of it.")
  );
}

#[test]
fn outline_text_ignores_a_trailing_comment_on_a_code_line() {
  let lines = [
    "/** Max retries. */",
    "export const RETRIES = 3;",
    "const WAIT = 50; /* ms */",
    "export function go() {}",
  ];
  let found = at(&lines, 3, 3, "go", Language::Script);
  assert_eq!((found.start, found.doc), (4, None));
}

#[test]
fn outline_text_keeps_a_declaration_after_an_inline_decorator() {
  let lines = [
    "  @Input() set value(v: string) {",
    "    this.v = v;",
    "  }",
    "function* worker() {}",
  ];
  assert_eq!(
    at(&lines, 0, 2, "value", Language::Script).signature,
    "set value(v: string)"
  );
  let generator = at(&lines, 3, 3, "worker", Language::Script);
  assert_eq!(
    (generator.signature.as_str(), generator.public),
    ("function* worker()", false)
  );
}
