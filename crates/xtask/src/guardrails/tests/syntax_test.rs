use super::{check, has_ident, is_cfg_test, is_test_fn, path_value, string_literal, test_fns};
use crate::guardrails::tests::{context, tree};

fn attr(text: &str) -> syn::Attribute {
  let item: syn::ItemMod = syn::parse_str(&format!("{text} mod m;")).unwrap();
  item.attrs.into_iter().next().unwrap()
}

#[test]
fn a_file_that_does_not_parse_is_a_finding_with_its_line() {
  let broken = tree(&[("crates/demo/src/lib.rs", "//! demo\n\nfn (\n")]);
  let found = check(&context(&broken.workspace));
  assert_eq!(found.len(), 1);
  assert_eq!((found[0].rule, found[0].line), ("parse", 3));
}

#[test]
fn cfg_test_is_recognised_inside_combinators_only_for_cfg() {
  assert!(is_cfg_test(&attr("#[cfg(test)]")));
  assert!(is_cfg_test(&attr("#[cfg(all(test, unix))]")));
  assert!(!is_cfg_test(&attr("#[cfg(unix)]")));
  assert!(!is_cfg_test(&attr("#[doc = \"test\"]")));
  assert!(!is_cfg_test(&attr("#[cfg_attr(test, path = \"x.rs\")]")));
}

#[test]
fn path_values_and_string_literals_are_read() {
  assert_eq!(
    path_value(&attr("#[path = \"tests/a_test.rs\"]")).as_deref(),
    Some("tests/a_test.rs")
  );
  assert_eq!(path_value(&attr("#[doc = \"x\"]")), None);
  assert_eq!(path_value(&attr("#[cfg(test)]")), None);
  assert_eq!(path_value(&attr("#[path = 1]")), None);
  assert_eq!(
    string_literal(&syn::parse_str("\"s\"").unwrap()).as_deref(),
    Some("s")
  );
  assert_eq!(string_literal(&syn::parse_str("1").unwrap()), None);
}

#[test]
fn identifiers_are_found_at_any_depth_but_not_in_literals() {
  let tokens: proc_macro2::TokenStream = "a(b(c), \"d\")".parse().unwrap();
  assert!(has_ident(&tokens, "c"));
  assert!(!has_ident(&tokens, "d"));
}

#[test]
fn test_functions_are_collected_with_their_ignore_flag() {
  let file: syn::File = syn::parse_str(
    "#[test] fn a() {}\n#[test]\n#[ignore]\nfn b() {}\nfn c() {}\nmod m { #[tokio::test] fn d() {} }\n",
  )
  .unwrap();
  let found: Vec<(String, bool)> = test_fns(&file)
    .into_iter()
    .map(|test| (test.name, test.ignored))
    .collect();
  assert_eq!(
    found,
    [
      ("a".to_owned(), false),
      ("b".to_owned(), true),
      ("d".to_owned(), false)
    ]
  );
  assert!(!is_test_fn(&[]));
}
