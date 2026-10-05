//! Parse failures, and the syntax helpers the rules share.

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::visit::Visit;
use syn::{Attribute, Meta};

use super::{Context, Finding};

/// A file that does not parse cannot be judged; that is itself a finding.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  ctx
    .sources
    .iter()
    .filter_map(|source| match &source.ast {
      Err((line, message)) => Some(Finding::new(
        "parse",
        &source.display(),
        *line,
        format!("not valid Rust: {message}"),
      )),
      Ok(_) => None,
    })
    .collect()
}

/// The 1-based line a span starts on.
pub(super) fn line(span: Span) -> usize {
  span.start().line
}

/// Whether `tokens` holds the identifier `name` at any depth.
pub(super) fn has_ident(tokens: &TokenStream, name: &str) -> bool {
  tokens.clone().into_iter().any(|tree| match tree {
    TokenTree::Ident(ident) => ident == name,
    TokenTree::Group(group) => has_ident(&group.stream(), name),
    TokenTree::Punct(_) | TokenTree::Literal(_) => false,
  })
}

/// `#[cfg(test)]`, or a `cfg` combinator that mentions `test`.
pub(super) fn is_cfg_test(attr: &Attribute) -> bool {
  let Meta::List(list) = &attr.meta else {
    return false;
  };
  list.path.is_ident("cfg") && has_ident(&list.tokens, "test")
}

/// The value of `#[path = "…"]`.
pub(super) fn path_value(attr: &Attribute) -> Option<String> {
  let Meta::NameValue(pair) = &attr.meta else {
    return None;
  };
  if !pair.path.is_ident("path") {
    return None;
  }
  string_literal(&pair.value)
}

/// The value of a string literal expression.
pub(super) fn string_literal(expr: &syn::Expr) -> Option<String> {
  let syn::Expr::Lit(syn::ExprLit {
    lit: syn::Lit::Str(text),
    ..
  }) = expr
  else {
    return None;
  };
  Some(text.value())
}

/// Whether `attrs` mark a test function (`#[test]` or `#[…::test]`).
pub(super) fn is_test_fn(attrs: &[Attribute]) -> bool {
  attrs.iter().any(|attr| {
    attr
      .path()
      .segments
      .last()
      .is_some_and(|segment| segment.ident == "test")
  })
}

/// A test function: its name and whether it carries `#[ignore]`.
pub(super) struct TestFn {
  pub(super) name: String,
  pub(super) ignored: bool,
}

/// Every test function in `file`, nested modules included.
pub(super) fn test_fns(file: &syn::File) -> Vec<TestFn> {
  struct Collect(Vec<TestFn>);
  impl<'ast> Visit<'ast> for Collect {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
      if is_test_fn(&item.attrs) {
        self.0.push(TestFn {
          name: item.sig.ident.to_string(),
          ignored: item.attrs.iter().any(|attr| attr.path().is_ident("ignore")),
        });
      }
      syn::visit::visit_item_fn(self, item);
    }
  }
  let mut collect = Collect(Vec::new());
  collect.visit_file(file);
  collect.0
}

#[cfg(test)]
#[path = "tests/syntax_test.rs"]
mod tests;
