//! The inline ast-grep rules that match outline items, one document per parser.
//!
//! Every branch captures the item's name as `$NAME`; the shared `not: inside`
//! drops items declared inside a function body, so methods stay and locals go.

use std::path::Path;

/// A language `outline` reads, chosen by file extension as ast-grep does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Language {
  /// `.rs`
  Rust,
  /// `.ts`, `.mts`, `.cts`, `.tsx`, `.js`, `.mjs`, `.cjs`, `.jsx`
  Script,
  /// `.py`, `.pyi`
  Python,
}

/// The language of `path`, or `None` for an extension outline does not read.
pub(super) fn language(path: &str) -> Option<Language> {
  match Path::new(path).extension()?.to_str()? {
    "rs" => Some(Language::Rust),
    "ts" | "mts" | "cts" | "tsx" | "js" | "mjs" | "cjs" | "jsx" => Some(Language::Script),
    "py" | "pyi" => Some(Language::Python),
    _ => None,
  }
}

const RUST: &str = "id: outline-rust
language: rust
rule:
  any:
    - any: [{kind: function_item}, {kind: function_signature_item}, {kind: struct_item}, {kind: enum_item},
            {kind: union_item}, {kind: trait_item}, {kind: type_item}, {kind: const_item},
            {kind: static_item}, {kind: mod_item}, {kind: macro_definition}]
      has: {field: name, pattern: $NAME}
    - kind: impl_item
      has: {field: type, pattern: $NAME}
  not:
    inside: {any: [{kind: function_item}, {kind: closure_expression}], stopBy: end}
";

const PYTHON: &str = "id: outline-python
language: python
rule:
  any: [{kind: function_definition}, {kind: class_definition}]
  has: {field: name, pattern: $NAME}
  not:
    inside: {kind: function_definition, stopBy: end}
";

/// Kinds JavaScript shares with TypeScript.
const SCRIPT_KINDS: &str = "{kind: function_declaration}, {kind: generator_function_declaration}, \
{kind: class_declaration}, {kind: method_definition}";

/// Kinds only TypeScript has.
const TYPE_KINDS: &str = ", {kind: abstract_class_declaration}, {kind: interface_declaration}, \
{kind: type_alias_declaration}, {kind: enum_declaration}, {kind: internal_module}";

fn script(language: &str, kinds: &str) -> String {
  format!(
    "id: outline-{language}
language: {language}
rule:
  any:
    - any: [{SCRIPT_KINDS}{kinds}]
      has: {{field: name, pattern: $NAME}}
    - kind: variable_declarator
      has: {{field: name, pattern: $NAME}}
      inside: {{kind: lexical_declaration, any: [{{inside: {{kind: program}}}}, {{inside: {{kind: export_statement}}}}]}}
  not:
    inside:
      any: [{{kind: function_declaration}}, {{kind: generator_function_declaration}}, {{kind: method_definition}},
            {{kind: arrow_function}}, {{kind: function_expression}}]
      stopBy: end
"
  )
}

/// All rule documents, for one scan over files of any supported language.
pub(super) fn rules() -> String {
  [
    RUST.to_owned(),
    script("typescript", TYPE_KINDS),
    script("tsx", TYPE_KINDS),
    script("javascript", ""),
    PYTHON.to_owned(),
  ]
  .join("---\n")
}

#[cfg(test)]
#[path = "tests/rules_test.rs"]
mod tests;
