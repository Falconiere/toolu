use std::path::Path;

use toolu_engine::outline::FileOutline;

use super::{Listed, text};

#[test]
fn large_map_bounds_directory_rows_and_counts_omissions() {
  let outlines: Vec<FileOutline> = (0..160)
    .map(|i| FileOutline {
      path: format!("d{i}/one.rs"),
      lines: 1,
      items: Vec::new(),
    })
    .collect();
  let files: Vec<Listed<'_>> = outlines
    .iter()
    .map(|file| (file.path.as_str(), Some(file)))
    .collect();
  let output = text(".", &files, Path::new("."));
  assert_eq!(output.lines().count(), 150);
  assert!(output.contains(".: 160 files, 160 directories"));
  assert!(output.contains("... 12 more directories"));
}
