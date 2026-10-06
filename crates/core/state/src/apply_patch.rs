//! Codex `apply_patch` bodies as edit records (`applyPatchRecords` in
//! `edit-records.ts`): every path a patch touches, or nothing when the patch
//! is malformed, so a pre-tool gate can fail closed. A move yields the source
//! (`update` with `moved_to`) and the destination (`move` with `from`).

use crate::edit_records::{EditOperation, EditRecord};

/// The walk over a patch's lines.
#[derive(Default)]
struct PatchState {
  records: Vec<EditRecord>,
  pending: String,
  begun: bool,
  ended: bool,
  headers: usize,
  invalid: bool,
}

/// Non-empty, one line, no tab: `_toolu_patch_path_valid`.
pub(crate) fn path_valid(path: &str) -> bool {
  !path.is_empty() && !path.contains(['\n', '\r', '\t'])
}

fn record(path: &str, operation: EditOperation) -> EditRecord {
  EditRecord {
    path: path.to_owned(),
    operation,
    moved_to: None,
    from: None,
  }
}

/// The path after `<prefix>`: exactly one space, then a valid path.
fn header_path<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
  let path = line.strip_prefix(prefix)?.strip_prefix(' ')?;
  path_valid(path).then_some(path)
}

impl PatchState {
  fn flush_pending(&mut self) {
    if !self.pending.is_empty() {
      let pending = std::mem::take(&mut self.pending);
      self.records.push(record(&pending, EditOperation::Update));
    }
  }

  /// `*** Add File:` or `*** Delete File:`.
  fn file_header(&mut self, line: &str, prefix: &str, operation: EditOperation) {
    if !self.begun {
      self.invalid = true;
      return;
    }
    self.flush_pending();
    match header_path(line, prefix) {
      Some(path) => {
        self.records.push(record(path, operation));
        self.headers += 1;
      }
      None => self.invalid = true,
    }
  }

  fn update_header(&mut self, line: &str) {
    if !self.begun {
      self.invalid = true;
      return;
    }
    self.flush_pending();
    match header_path(line, "*** Update File:") {
      Some(path) => {
        path.clone_into(&mut self.pending);
        self.headers += 1;
      }
      None => self.invalid = true,
    }
  }

  fn move_header(&mut self, line: &str) {
    let target = (self.begun && !self.pending.is_empty())
      .then(|| header_path(line, "*** Move to:"))
      .flatten();
    let Some(target) = target else {
      self.invalid = true;
      return;
    };
    let source = std::mem::take(&mut self.pending);
    self.records.push(EditRecord {
      moved_to: Some(target.to_owned()),
      ..record(&source, EditOperation::Update)
    });
    self.records.push(EditRecord {
      from: Some(source),
      ..record(target, EditOperation::Move)
    });
    self.headers += 1;
  }

  fn line(&mut self, line: &str) {
    if self.ended {
      self.invalid |= !line.is_empty();
    } else if line == "*** Begin Patch" {
      self.invalid |= self.begun;
      self.begun = true;
    } else if line == "*** End Patch" {
      self.end();
    } else if line.starts_with("*** Add File:") {
      self.file_header(line, "*** Add File:", EditOperation::Add);
    } else if line.starts_with("*** Update File:") {
      self.update_header(line);
    } else if line.starts_with("*** Delete File:") {
      self.file_header(line, "*** Delete File:", EditOperation::Delete);
    } else if line.starts_with("*** Move to:") {
      self.move_header(line);
    } else if line == "*** End of File" {
      // An optional end-of-file marker inside a file operation; it names no path.
      self.invalid |= !self.begun || self.headers == 0;
    } else if line.starts_with("*** ") {
      // An unknown control header may carry a path a newer grammar added: never skip it.
      self.invalid = true;
    }
  }

  fn end(&mut self) {
    if self.begun {
      self.flush_pending();
      self.ended = true;
    } else {
      self.invalid = true;
    }
  }
}

/// `_toolu_apply_patch_records`: every affected path, or `None` when malformed.
pub fn apply_patch_records(patch: &str) -> Option<Vec<EditRecord>> {
  let mut state = PatchState::default();
  // A here-string feeds `read` one line per "\n"; each line loses one trailing CR.
  for line in patch.split('\n') {
    state.line(line.strip_suffix('\r').unwrap_or(line));
  }
  let valid = !state.invalid && state.begun && state.ended && state.headers > 0;
  valid.then_some(state.records)
}

#[cfg(test)]
#[path = "tests/apply_patch_test.rs"]
mod tests;
