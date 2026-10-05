//! Attributes and macros: no suppression (rule 20), no `#[ignore]` (rule 10),
//! no source includes and `#[path]` only for test wiring (rule 11).

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Meta};

use super::syntax::{has_ident, is_cfg_test, line, path_value};
use super::{Context, Finding};
use crate::source::{Kind, Source};

/// Every banned attribute, macro and suppression comment.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found = Vec::new();
  for source in &ctx.sources {
    let path = source.display();
    for (at, text) in &source.lines.comments {
      if text.contains("jscpd:ignore") {
        found.push(Finding::new(
          "suppression",
          &path,
          *at,
          "jscpd ignore comment — remove the duplication instead".to_owned(),
        ));
      }
    }
    let Ok(ast) = &source.ast else { continue };
    let mut walk = Walk {
      ctx,
      source,
      found: Vec::new(),
    };
    walk.visit_file(ast);
    found.extend(walk.found);
  }
  found
}

struct Walk<'c, 'w> {
  ctx: &'c Context<'w>,
  source: &'c Source<'w>,
  found: Vec<Finding>,
}

impl Walk<'_, '_> {
  fn push(&mut self, rule: &'static str, at: usize, message: String) {
    self
      .found
      .push(Finding::new(rule, &self.source.display(), at, message));
  }

  /// Whether `value` on a `mod` is test wiring for this file.
  fn wiring(&self, module: &syn::ItemMod, value: &str) -> bool {
    let suffix = format!("{}.rs", self.ctx.rules.tests.file_suffix);
    match self.source.kind {
      Kind::Src | Kind::UnitTest => {
        module.content.is_none()
          && module.attrs.iter().any(is_cfg_test)
          && value
            .strip_prefix("tests/")
            .is_some_and(|file| !file.contains('/'))
          && value.ends_with(&suffix)
      }
      Kind::IntegrationTest => self
        .ctx
        .rules
        .tests
        .subdirs
        .iter()
        .filter(|dir| *dir != "fixtures")
        .any(|dir| value.starts_with(&format!("{dir}/"))),
      Kind::Other => false,
    }
  }
}

/// The suppressing lint level of `attr`, if any: `allow`, `expect`, or either inside `cfg_attr`.
fn suppression(attr: &Attribute) -> Option<&'static str> {
  ["allow", "expect"].into_iter().find(|level| {
    if let Meta::List(list) = &attr.meta
      && list.path.is_ident("cfg_attr")
    {
      return has_ident(&list.tokens, level);
    }
    attr.path().is_ident(level)
  })
}

impl<'ast> Visit<'ast> for Walk<'_, '_> {
  fn visit_attribute(&mut self, attr: &'ast Attribute) {
    let at = line(attr.span());
    if let Some(level) = suppression(attr) {
      self.push(
        "suppression",
        at,
        format!("`{level}` lint attribute — fix the code; a wrong rule is changed for everyone"),
      );
    }
    if attr.path().is_ident("ignore") {
      self.push(
        "no-ignore",
        at,
        "#[ignore] — a test that cannot run is deleted or fixed".to_owned(),
      );
    }
  }

  fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
    for attr in &module.attrs {
      if let Some(value) = path_value(attr)
        && !self.wiring(module, &value)
      {
        self.push(
          "structure",
          line(attr.span()),
          format!("#[path = \"{value}\"] is only for test wiring"),
        );
      }
    }
    syn::visit::visit_item_mod(self, module);
  }

  fn visit_macro(&mut self, mac: &'ast syn::Macro) {
    if mac
      .path
      .segments
      .last()
      .is_some_and(|segment| segment.ident == "include")
    {
      self.push(
        "structure",
        line(mac.span()),
        "include of a source file — declare a module instead".to_owned(),
      );
    }
    syn::visit::visit_macro(self, mac);
  }
}

#[cfg(test)]
#[path = "tests/attributes_test.rs"]
mod tests;
