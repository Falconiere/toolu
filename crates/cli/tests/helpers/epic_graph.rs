//! The committed #248 graph, written under a test root.

use serde_json::Value;

use crate::epic::{Home, Res};

/// The committed #248 graph.
const GRAPH: &str = include_str!("../../../../fixtures/epic/epic248-graph.json");

/// Write the #248 graph with its state under `home`, edited by `edit`; the file.
pub(crate) fn write_graph(home: &Home, edit: impl FnOnce(&mut Value)) -> Res<String> {
  let mut graph: Value = serde_json::from_str(GRAPH)?;
  let state = Value::String(home.path("state").display().to_string());
  graph
    .as_object_mut()
    .ok_or("the graph is not an object")?
    .insert("state_dir".to_owned(), state);
  edit(&mut graph);
  let file = home.path("graph.json");
  std::fs::write(&file, graph.to_string())?;
  Ok(file.display().to_string())
}
