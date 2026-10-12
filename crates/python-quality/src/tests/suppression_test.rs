use std::path::PathBuf;

use toolu_engine::quality::EditedFile;

use super::suppression;
use crate::file::{Limits, PyFile};

fn found(text: &str) -> Option<String> {
  let file = EditedFile {
    path: "s.py".to_owned(),
    absolute: PathBuf::from("/nonexistent/s.py"),
    removed: false,
  };
  suppression(&PyFile::new(&file, text, Limits::default()))
}

const PREFIX: &str = "Forbidden suppression in s.py — remove it and fix the underlying issue. \
  Scoped forms (except SpecificError:, # noqa: E501, # type: ignore[arg-type]) are fine; blanket \
  ones are not:\n";

#[test]
fn each_blanket_form_is_reported_with_its_line() {
  let text = "try:\n    x()\nexcept:  # all\n    pass\ntry:\n    y()\nexcept ValueError: pass  # ok\n\
    z = 1  # noqa\nw = 2  # type: ignore\n";
  assert_eq!(
    found(text),
    Some(format!(
      "{PREFIX}3: bare except: (swallows everything)\n7: one-line except ...: pass (silently \
       discarded)\n8: blanket # noqa (no :CODE suffix)\n9: blanket # type: ignore (no [code] \
       suffix)"
    ))
  );
}

#[test]
fn scoped_forms_pass_and_crlf_and_tabs_still_match() {
  let scoped =
    "except ValueError:\n    raise\nx = 1  # noqa: E501\ny = 2  # type: ignore[arg-type]\n";
  assert_eq!(found(scoped), None);
  assert_eq!(
    found("\texcept:\r\n"),
    Some(format!("{PREFIX}1: bare except: (swallows everything)"))
  );
  let many = "except:\n".repeat(7);
  let shown = found(&many).unwrap();
  assert!(
    shown.ends_with("\n5: bare except: (swallows everything)"),
    "{shown}"
  );
}
