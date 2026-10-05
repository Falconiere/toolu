//! Rule 15, the part no dependency tool sees: no type-erased error
//! (`dyn … Error`) in a library's public function signatures.

use syn::spanned::Spanned;
use syn::visit::Visit;

use super::syntax::line;
use super::{Context, Finding};
use crate::source::Kind;

/// Public functions, aliases, fields and trait methods of library crates that mention `dyn … Error`.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found = Vec::new();
  for source in &ctx.sources {
    let is_lib = source.member.is_some_and(|member| member.is_lib);
    if source.kind != Kind::Src || !is_lib {
      continue;
    }
    let Ok(ast) = &source.ast else { continue };
    let mut public = Public(Vec::new());
    public.visit_file(ast);
    for (at, name) in public.0 {
      found.push(Finding::new(
        "public-api",
        &source.display(),
        at,
        format!("pub {name} exposes a type-erased error (`dyn Error`); use a typed error"),
      ));
    }
  }
  found
}

/// Public items whose types name a `dyn` trait ending in `Error`.
struct Public(Vec<(usize, String)>);

impl Public {
  fn sig(&mut self, vis: &syn::Visibility, sig: &syn::Signature) {
    if !matches!(vis, syn::Visibility::Public(_)) {
      return;
    }
    let mut erased = Erased(false);
    erased.visit_signature(sig);
    if erased.0 {
      self.0.push((line(sig.span()), sig.ident.to_string()));
    }
  }
}

impl Public {
  /// A public item `name` whose types `types` name a type-erased error.
  fn types<'a>(
    &mut self,
    at: usize,
    name: &syn::Ident,
    types: impl IntoIterator<Item = &'a syn::Type>,
  ) {
    let mut erased = Erased(false);
    for ty in types {
      erased.visit_type(ty);
    }
    if erased.0 {
      self.0.push((at, name.to_string()));
    }
  }
}

fn is_pub(vis: &syn::Visibility) -> bool {
  matches!(vis, syn::Visibility::Public(_))
}

impl<'ast> Visit<'ast> for Public {
  fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
    if is_pub(&item.vis) {
      self.types(line(item.ident.span()), &item.ident, [item.ty.as_ref()]);
    }
  }

  fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
    if is_pub(&item.vis) {
      let fields = item.fields.iter().filter(|field| is_pub(&field.vis));
      self.types(
        line(item.ident.span()),
        &item.ident,
        fields.map(|field| &field.ty),
      );
    }
  }

  fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
    if is_pub(&item.vis) {
      let fields = item
        .variants
        .iter()
        .flat_map(|variant| variant.fields.iter());
      self.types(
        line(item.ident.span()),
        &item.ident,
        fields.map(|field| &field.ty),
      );
    }
  }

  fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
    if !is_pub(&item.vis) {
      return;
    }
    for inner in &item.items {
      if let syn::TraitItem::Fn(method) = inner {
        self.sig(&item.vis, &method.sig);
      }
    }
  }

  fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
    self.sig(&item.vis, &item.sig);
  }

  fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
    self.sig(&item.vis, &item.sig);
  }
}

struct Erased(bool);

impl<'ast> Visit<'ast> for Erased {
  fn visit_type_trait_object(&mut self, object: &'ast syn::TypeTraitObject) {
    self.0 |= object.bounds.iter().any(|bound| {
      let syn::TypeParamBound::Trait(bound) = bound else {
        return false;
      };
      bound
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Error")
    });
    syn::visit::visit_type_trait_object(self, object);
  }
}

#[cfg(test)]
#[path = "tests/public_api_test.rs"]
mod tests;
