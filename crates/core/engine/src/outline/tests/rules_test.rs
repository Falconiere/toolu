use super::{Language, language, rules};

#[test]
fn outline_rules_language_follows_the_extension() {
  assert_eq!(language("src/a.rs"), Some(Language::Rust));
  assert_eq!(language("web/b.tsx"), Some(Language::Script));
  assert_eq!(language("c.mjs"), Some(Language::Script));
  assert_eq!(language("d.pyi"), Some(Language::Python));
  assert_eq!(language("README.md"), None);
  assert_eq!(language("Makefile"), None);
}

#[test]
fn outline_rules_hold_one_document_per_parser() {
  let text = rules();
  let ids: Vec<&str> = text
    .lines()
    .filter_map(|line| line.strip_prefix("id: "))
    .collect();
  assert_eq!(
    ids,
    [
      "outline-rust",
      "outline-typescript",
      "outline-tsx",
      "outline-javascript",
      "outline-python"
    ]
  );
  assert_eq!(text.matches("\n---\n").count(), 4);
  assert!(
    !text
      .split("id: outline-javascript")
      .nth(1)
      .unwrap_or_default()
      .split("---")
      .next()
      .unwrap_or_default()
      .contains("interface")
  );
}
