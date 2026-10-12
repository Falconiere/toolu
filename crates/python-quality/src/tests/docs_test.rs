use std::path::PathBuf;

use toolu_engine::quality::EditedFile;

use super::docs;
use crate::file::{Limits, PyFile};

fn advice(text: &str) -> String {
  let file = EditedFile {
    path: "d.py".to_owned(),
    absolute: PathBuf::from("/nonexistent/d.py"),
    removed: false,
  };
  docs(&PyFile::new(&file, text, Limits::default()))
}

const HEAD: &str = "Public def/class missing a docstring in d.py — add a concise one:\n";

#[test]
fn a_public_top_level_definition_needs_a_docstring() {
  assert_eq!(advice("def a():\n    return 1\n"), format!("{HEAD}1: a"));
  assert_eq!(advice("class C:\n\n    x = 1\n"), format!("{HEAD}1: C"));
  for documented in [
    "def a():\n    \"\"\"Doc.\"\"\"\n",
    "async def a():\n\n    '''Doc.'''\n",
    "def a(\n    x,\n):  # sig\n    r\"\"\"Doc.\"\"\"\n",
    "def _a():\n    return 1\n",
    "class C:\n    \"\"\"D.\"\"\"\n    def m(self):\n        return 1\n",
  ] {
    assert_eq!(advice(documented), "", "{documented:?}");
  }
  assert_eq!(
    advice("def a(\n    x):\n    return x\n"),
    format!("{HEAD}1: a")
  );
}

#[test]
fn a_consumed_body_line_and_the_cap_follow_the_awk_program() {
  assert_eq!(advice("def a(): pass\ndef b():\n    \"\"\"D.\"\"\"\n"), "");
  assert_eq!(
    advice("def a():  # c\n    # not a docstring\n"),
    format!("{HEAD}1: a")
  );
  let many = "def a():\n    x\ndef b():\n    x\ndef c():\n    x\ndef d():\n    x\n";
  assert_eq!(advice(many), format!("{HEAD}1: a\n3: b\n5: c"));
  assert_eq!(advice("def a():\r\n    \"\"\"D.\"\"\"\r\n"), "");
}
