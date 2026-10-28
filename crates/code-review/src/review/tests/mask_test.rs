use super::{MASK, Mask};
use crate::contract::{Coverage, RedactionSpan, Side};
use crate::prepass::Redaction;
use crate::shape::{ChangedLine, FileKind, Shaped, ShapedFile};

fn shaped(line: &str) -> Shaped {
  Shaped {
    base_ref: "main".into(),
    merge_base: "abc".into(),
    diff_sha: "def".into(),
    gate_files: vec!["src/a.rs".into()],
    files: vec![ShapedFile {
      path: "src/a.rs".into(),
      old_path: None,
      kind: FileKind::Modified,
      patch: format!("@@ -1 +1 @@\n-old\n+{line}\n"),
      lines: vec![ChangedLine {
        side: Side::New,
        line: 1,
        text: line.into(),
      }],
    }],
    chunks: Vec::new(),
    coverage: Coverage::default(),
    renames: Vec::new(),
    uncommitted: false,
  }
}

fn redaction(line: u32, start: usize, end: usize) -> Redaction {
  Redaction {
    path: "src/a.rs".into(),
    side: Side::New,
    line,
    spans: vec![RedactionSpan { start, end }],
  }
}

#[test]
fn a_secret_is_masked_everywhere_it_appears() {
  let text = "let key = \"sk-live-0123456789abcdef\";";
  let mask = Mask::new(&shaped(text), &[redaction(1, 11, 35)]).unwrap();
  let outbound = format!("patch +{text}\nsource {text}\nquote sk-live-0123456789abcdef");
  let masked = mask.apply(&outbound);
  assert!(!masked.contains("sk-live"), "{masked}");
  assert_eq!(masked.matches(MASK).count(), 3);
  assert_eq!(Mask::default().apply(text), text);
}

#[test]
fn a_redaction_outside_the_diff_refuses_to_build_a_mask() {
  let shaped = shaped("let x = 1;");
  assert!(Mask::new(&shaped, &[redaction(9, 0, 3)]).is_err());
  assert!(Mask::new(&shaped, &[redaction(1, 4, 99)]).is_err());
  assert!(Mask::new(&shaped, &[redaction(1, 2, 2)]).is_err());
}
