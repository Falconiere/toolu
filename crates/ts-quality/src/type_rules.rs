//! Duplicate exported types (50) and throws of non-Error literals (80). The
//! duplicate lookup is the same `git grep` the bash fragment ran, so git's index
//! and pathspec rules decide which files count.

use toolu_runtime::process::{Spec, run};

use crate::file::{Pattern, TsFile, head};

const PATHSPECS: [&str; 4] = ["packages/*.ts", "packages/*.tsx", "apps/*.ts", "apps/*.tsx"];

/// `git grep -l --untracked` for another file exporting `name`, or "".
fn defined_elsewhere(f: &TsFile<'_>, name: &str, relative: &str) -> String {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(f.at.project_root.display().to_string());
  spec.argv.extend(
    ["grep", "-l", "--untracked", "-E"]
      .into_iter()
      .map(str::to_owned),
  );
  spec
    .argv
    .push(format!("^export (interface|type) {name}[ <{{]"));
  spec.argv.push("--".to_owned());
  spec.argv.extend(PATHSPECS.map(str::to_owned));
  spec.cwd = Some(f.at.cwd.to_path_buf());
  spec.env = Some(f.at.env.clone());
  let Ok(output) = run(&spec) else {
    return String::new();
  };
  output
    .stdout
    .split('\n')
    .find(|line| !line.is_empty() && *line != relative)
    .unwrap_or_default()
    .to_owned()
}

static EXPORTED_TYPE: Pattern = Pattern::new("^export (?:interface|type) ([A-Z][a-zA-Z]+)");

/// 50-type-dup: one error per exported type name another package already exports.
pub(crate) fn duplicate_types(f: &TsFile<'_>) -> Vec<String> {
  let names: Vec<&str> = f
    .lines
    .iter()
    .filter_map(|line| EXPORTED_TYPE.first_group(line))
    .collect();
  if names.is_empty() {
    return Vec::new();
  }
  let path = f.path();
  let prefix = format!("{}/", f.at.project_root.display());
  let relative = path.strip_prefix(&prefix).unwrap_or(path);
  names
    .into_iter()
    .filter_map(|name| {
      let other = defined_elsewhere(f, name, relative);
      (!other.is_empty()).then(|| {
        format!("Type '{name}' in {path} already defined in {other} — import instead of redefining")
      })
    })
    .collect()
}

static THROW_LITERAL: Pattern = Pattern::new(
  r"(^|[^a-zA-Z_$])throw[ \t]+(-?[0-9]+(\.[0-9]+)?|null|undefined|true|false)([ \t]|;|}|$)",
);
static BLOCK_COMMENT: Pattern = Pattern::new(r"/\*.*\*/");
static LINE_COMMENT: Pattern = Pattern::new("//.*$");

/// 80-throw-literal: `/* … */` and `//` comments are stripped before matching.
pub(crate) fn throw_literal(f: &TsFile<'_>) -> Option<String> {
  let rows: Vec<String> = f
    .lines
    .iter()
    .enumerate()
    .filter(|(_, line)| {
      let code = LINE_COMMENT.remove_all(&BLOCK_COMMENT.remove_all(line));
      THROW_LITERAL.is_match(&code)
    })
    .map(|(at, line)| format!("{}: {line}", at + 1))
    .collect();
  let excerpt = head(&rows, 3);
  (!excerpt.is_empty()).then(|| {
    format!(
      "throw of non-Error literal in {} — throw an Error (or subclass) instead\n{excerpt}",
      f.path()
    )
  })
}

#[cfg(test)]
#[path = "tests/type_rules_test.rs"]
mod tests;
