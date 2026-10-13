//! Forbidden suppression markers (30-suppression): bare `except:`, a one-line
//! `except …: pass`, blanket `# noqa` and blanket `# type: ignore`. Each line
//! reports its first matching form, and the first five are shown.

use crate::file::{Ere, PyFile, ere, head, matches};

static BARE: Ere = Ere::new(|| ere(r"^[[:space:]]*except[[:space:]]*:[[:space:]]*(#.*)?$"));
static PASS: Ere =
  Ere::new(|| ere(r"^[[:space:]]*except[^:]*:[[:space:]]*pass[[:space:]]*(#.*)?$"));
static NOQA: Ere = Ere::new(|| ere(r"#[ \t]*noqa"));
static SCOPED_NOQA: Ere = Ere::new(|| ere(r"#[ \t]*noqa[ \t]*:[ \t]*[A-Za-z0-9]"));
static IGNORE: Ere = Ere::new(|| ere(r"#[ \t]*type:[ \t]*ignore"));
static SCOPED_IGNORE: Ere = Ere::new(|| ere(r"#[ \t]*type:[ \t]*ignore\["));

fn form(line: &str) -> Option<&'static str> {
  if matches(&BARE, line) {
    return Some("bare except: (swallows everything)");
  }
  if matches(&PASS, line) {
    return Some("one-line except ...: pass (silently discarded)");
  }
  if matches(&NOQA, line) && !matches(&SCOPED_NOQA, line) {
    return Some("blanket # noqa (no :CODE suffix)");
  }
  if matches(&IGNORE, line) && !matches(&SCOPED_IGNORE, line) {
    return Some("blanket # type: ignore (no [code] suffix)");
  }
  None
}

/// The suppression violation of `f`, if any line holds a forbidden form.
pub(crate) fn suppression(f: &PyFile<'_>) -> Option<String> {
  let hits: Vec<String> = f
    .lines
    .iter()
    .enumerate()
    .filter_map(|(index, line)| form(line).map(|found| format!("{}: {found}", index + 1)))
    .collect();
  (!hits.is_empty()).then(|| {
    format!(
      "Forbidden suppression in {} — remove it and fix the underlying issue. Scoped forms \
       (except SpecificError:, # noqa: E501, # type: ignore[arg-type]) are fine; blanket ones \
       are not:\n{}",
      f.file.path,
      head(&hits, 5)
    )
  })
}

#[cfg(test)]
#[path = "tests/suppression_test.rs"]
mod tests;
