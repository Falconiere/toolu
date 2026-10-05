//! One Rust file of the workspace: where it sits, its lines and its syntax tree.

use std::path::{Path, PathBuf};

use crate::lexer::{self, Lines};
use crate::workspace::{Member, Workspace, parts};

/// Where a Rust file sits in its crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  /// Production code under `src/`.
  Src,
  /// A unit-test file in a `tests/` directory under `src/`.
  UnitTest,
  /// A black-box test file under the crate's `tests/`.
  IntegrationTest,
  /// Anything else, such as an admitted fuzz package.
  Other,
}

/// A parsed Rust file.
pub(crate) struct Source<'w> {
  /// Path relative to the workspace root.
  pub(crate) rel: PathBuf,
  /// The member that holds the file, if any.
  pub(crate) member: Option<&'w Member>,
  pub(crate) kind: Kind,
  pub(crate) lines: Lines,
  /// The syntax tree, or the parse error with its line.
  pub(crate) ast: Result<syn::File, (usize, String)>,
}

impl<'w> Source<'w> {
  /// Read, lex and parse `rel`.
  pub(crate) fn load(workspace: &'w Workspace, rel: &Path) -> Result<Self, String> {
    let text = workspace.read(rel)?;
    let member = workspace.member_of(rel);
    Ok(Self::new(rel, member, &text))
  }

  /// Lex and parse `text` as the file `rel` of `member`.
  pub(crate) fn new(rel: &Path, member: Option<&'w Member>, text: &str) -> Self {
    let kind = member.map_or(Kind::Other, |member| kind_of(&member.dir, rel));
    let ast = syn::parse_file(text).map_err(|err| (err.span().start().line, err.to_string()));
    Source {
      rel: rel.to_path_buf(),
      member,
      kind,
      lines: lexer::lex(text),
      ast,
    }
  }

  /// The path with `/` separators, for messages.
  pub(crate) fn display(&self) -> String {
    self.rel.to_string_lossy().replace('\\', "/")
  }

  /// The path relative to the crate's `src/`, when the file is under it.
  pub(crate) fn in_src(&self) -> Option<PathBuf> {
    let member = self.member?;
    self
      .rel
      .strip_prefix(member.dir.join("src"))
      .ok()
      .map(Path::to_path_buf)
  }

  /// Whether the file is test code of any kind.
  pub(crate) fn is_test(&self) -> bool {
    matches!(self.kind, Kind::UnitTest | Kind::IntegrationTest)
  }
}

/// Classify `rel` inside the member directory `dir`.
fn kind_of(dir: &Path, rel: &Path) -> Kind {
  let Ok(inside) = rel.strip_prefix(dir) else {
    return Kind::Other;
  };
  let names = parts(inside);
  let Some((first, rest)) = names.split_first() else {
    return Kind::Other;
  };
  let in_tests_dir = rest.iter().rev().skip(1).any(|dir| dir == "tests");
  match first.as_str() {
    "src" if in_tests_dir => Kind::UnitTest,
    "src" => Kind::Src,
    "tests" => Kind::IntegrationTest,
    _ => Kind::Other,
  }
}

#[cfg(test)]
#[path = "tests/source_test.rs"]
mod tests;
