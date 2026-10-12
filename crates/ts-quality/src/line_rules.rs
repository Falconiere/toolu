//! The grep-based rules, each a port of one bash fragment: `as` assertions,
//! React hooks, factories, type guards, component file names, `console.log`,
//! suppression comments, confirm/alert, raw radix imports, mutable props and
//! try/catch+toast. Each returns its violation text, or `None`.

use std::path::Path;

use crate::file::{
  Pattern, TsFile, count_matches, has_extension, head, is_test_path, numbered, with_excerpt, without_comment_lines,
};

static AS_PATTERN: Pattern = Pattern::new(
  r"\)[[:space:]]+as[[:space:]]+[a-zA-Z]|\bas[[:space:]]+any\b|\bas[[:space:]]+unknown\b|[a-zA-Z>][[:space:]]+as[[:space:]]+[A-Z]|[a-zA-Z>][[:space:]]+as[[:space:]]+(string|number|boolean|object|symbol|bigint|never|undefined|null|void)\b",
);
static AS_CONST: Pattern = Pattern::new(r"\bas[[:space:]]+const\b");
static AS_IMPORT_OR_REEXPORT: Pattern =
  Pattern::new(r"\bimport\b|^[0-9]+:[[:space:]]*export[[:space:]]*(type[[:space:]]+)?\{");

/// 15-type-as.
pub(crate) fn type_assertion(f: &TsFile<'_>) -> Option<String> {
  let rows: Vec<String> = without_comment_lines(numbered(&f.lines, &AS_PATTERN))
    .into_iter()
    .filter(|row| !AS_CONST.is_match(row) && !AS_IMPORT_OR_REEXPORT.is_match(row))
    .collect();
  let header = format!(
    "Forbidden 'as' type assertion in {} — use type guards or Zod",
    f.path()
  );
  with_excerpt(&header, &head(&rows, 5))
}

static HOOK_FILE_DASH: Pattern = Pattern::new(r"use-.*\.ts$");
static HOOK_FILE_CAMEL: Pattern = Pattern::new(r"use[A-Z].*\.ts$");
static HOOK_CALL: Pattern = Pattern::new(r"^[[:space:]]*(const \[|useRef\(|useEffect\()");

/// 35-react-hooks.
pub(crate) fn react_hooks(f: &TsFile<'_>) -> Option<String> {
  if !HOOK_FILE_DASH.is_match(f.path()) && !HOOK_FILE_CAMEL.is_match(f.path()) {
    return None;
  }
  let count = count_matches(&f.lines, &HOOK_CALL);
  (count > 3).then(|| {
    format!(
      "Hook does too many things in {} ({count} useState/useRef/useEffect) — split into focused hooks",
      f.path()
    )
  })
}

static FACTORY: Pattern = Pattern::new(r"^export (async )?function create");

/// 40-factory.
pub(crate) fn factories(f: &TsFile<'_>) -> Option<String> {
  let count = count_matches(&f.lines, &FACTORY);
  (count > 2).then(|| {
    format!(
      "Too many factory functions in {} ({count}) — simplify construction",
      f.path()
    )
  })
}

fn project_uses_zod(root: &Path) -> bool {
  let manifest = root.join("package.json");
  std::fs::metadata(&manifest).is_ok_and(|meta| meta.is_file())
    && std::fs::read(&manifest).is_ok_and(|bytes| {
      String::from_utf8_lossy(&bytes).contains("\"zod\"")
    })
}

static TYPE_GUARD: Pattern = Pattern::new(r"function is[A-Z].*\): .* is [A-Z]");

/// 45-typeguard.
pub(crate) fn manual_type_guard(f: &TsFile<'_>) -> Option<String> {
  if count_matches(&f.lines, &TYPE_GUARD) == 0 || !project_uses_zod(f.at.project_root) {
    return None;
  }
  Some(format!(
    "Manual type guard in {} — use Zod schema instead",
    f.path()
  ))
}

static GRAB_BAG: Pattern =
  Pattern::new(r"^(parts|components|helpers|items|sections|elements)$|-(parts|sections|items|elements)$");

/// 55-naming: a `.tsx` named after a grab-bag, not its exported component.
pub(crate) fn component_file_name(f: &TsFile<'_>) -> Option<String> {
  let path = f.path();
  if !has_extension(path, &["tsx"]) {
    return None;
  }
  let base = path.rsplit('/').next().unwrap_or(path);
  let name = base.strip_suffix(".tsx").unwrap_or(base);
  let name = name.strip_suffix(".ts").unwrap_or(name);
  GRAB_BAG.is_match(name).then(|| {
    format!(
      "Forbidden component filename '{name}.tsx' in {path} — name file after its exported function (e.g. api-key-create-button.tsx)"
    )
  })
}

static CONSOLE_LOG: Pattern = Pattern::new(r"^[[:space:]]*console\.log\(");

/// 60-console.
pub(crate) fn console_log(f: &TsFile<'_>) -> Option<String> {
  let rows = without_comment_lines(numbered(&f.lines, &CONSOLE_LOG));
  let header = format!(
    "Forbidden console.log in {} — use console.error/warn/info",
    f.path()
  );
  with_excerpt(&header, &head(&rows, 3))
}

static SUPPRESSION_TEST: Pattern =
  Pattern::new(r"(//|/\*+)[[:space:]]*(@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore)");
static SUPPRESSION: Pattern = Pattern::new(
  r"(//|/\*+)[[:space:]]*(@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore|@ts-expect-error)",
);

/// 65-suppression: `@ts-expect-error` is exempt only in test files.
pub(crate) fn suppression_comment(f: &TsFile<'_>) -> Option<String> {
  let pattern = if is_test_path(f.path()) {
    &SUPPRESSION_TEST
  } else {
    &SUPPRESSION
  };
  let rows = numbered(&f.lines, pattern);
  let header = format!(
    "Forbidden suppression comment in {} — fix the underlying issue in code, never silence it",
    f.path()
  );
  with_excerpt(&header, &head(&rows, 3))
}

fn is_frontend(f: &TsFile<'_>) -> bool {
  f.path().contains("/components/") || f.path().contains("/routes/")
}

static CONFIRM: Pattern = Pattern::new(r"\b(confirm|alert)[[:space:]]*\(");
static SHARED_ALERT: Pattern =
  Pattern::new("(ConfirmDeleteAlert|AlertDialog|customAlert|customConfirm)");

/// 70-ui-confirm.
pub(crate) fn confirm_alert(f: &TsFile<'_>) -> Option<String> {
  if !is_frontend(f) {
    return None;
  }
  let rows: Vec<String> = without_comment_lines(numbered(&f.lines, &CONFIRM))
    .into_iter()
    .filter(|row| !SHARED_ALERT.is_match(row))
    .collect();
  let header = format!(
    "Forbidden confirm()/alert() in {} — use AlertDialog component",
    f.path()
  );
  with_excerpt(&header, &head(&rows, 3))
}

static RADIX: Pattern = Pattern::new(r#"from ['"]@radix-ui/react-(alert-dialog|dialog)['"]"#);

/// 72-ui-radix.
pub(crate) fn raw_radix_import(f: &TsFile<'_>) -> Option<String> {
  let imports = without_comment_lines(numbered(&f.lines, &RADIX));
  if imports.is_empty() || f.path().contains("/packages/ui/") {
    return None;
  }
  Some(format!(
    "Raw radix import in {} — use shared components from @/components/ui/",
    f.path()
  ))
}

static PROPS: Pattern =
  Pattern::new(r"\((props|[a-z]+Props):[[:space:]]+[A-Z][a-zA-Z]+Props\)");

/// 74-react-props.
pub(crate) fn mutable_props(f: &TsFile<'_>) -> Option<String> {
  let rows: Vec<String> = numbered(&f.lines, &PROPS)
    .into_iter()
    .filter(|row| !row.contains("Readonly"))
    .collect();
  let header = format!("Mutable props in {} — wrap in Readonly<Props>", f.path());
  with_excerpt(&header, &head(&rows, 3))
}

static CATCH_OPEN: Pattern = Pattern::new(r"catch[[:space:]]*\(");

/// 76-toast: a `toast(` after a `catch (`, in a component or route.
pub(crate) fn catch_toast(f: &TsFile<'_>) -> Option<String> {
  if !is_frontend(f) {
    return None;
  }
  let mut rows = Vec::new();
  let mut found = false;
  for (at, line) in f.lines.iter().enumerate() {
    if CATCH_OPEN.is_match(line) {
      found = true;
    }
    if found && line.contains("toast(") {
      rows.push(format!("{}: {line}", at + 1));
      found = false;
    }
  }
  let header = format!(
    "Manual try/catch+toast in {} — use shared error handling",
    f.path()
  );
  with_excerpt(&header, &head(&rows, 3))
}

#[cfg(test)]
#[path = "tests/line_rules_test.rs"]
mod tests;
