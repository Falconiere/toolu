use serde_json::json;

use super::list;
use crate::disk::write_value;
use crate::model::World;
use crate::paths::Paths;

#[test]
fn empty_registered_epic_still_has_a_status_board() {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  write_value(
    &paths.registry(),
    &json!({
      "version": 1,
      "epics": [{"key": "repo-402", "state_dir": temp.path().join("epic")}]
    }),
  )
  .expect("registry");
  let items = list(&World::new(0), &paths).expect("epic list");
  assert_eq!(items.len(), 1);
  assert_eq!(items[0]["key"], "repo-402");
  assert_eq!(items[0]["counts"], json!({}));
}
