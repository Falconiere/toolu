use std::path::Path;

use toolu_engine::outline::FileOutline;

use super::{Listed, json, text};

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
  let document = json(".", &files, Path::new("."));
  assert_eq!(document["directories"].as_array().unwrap().len(), 148);
  assert_eq!(document["directories"][0]["lines"], 1);
  assert_eq!(document["directories"][0]["names"], serde_json::json!([]));
  assert_eq!(document["omitted"], 12);
}
