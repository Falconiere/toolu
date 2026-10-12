//! The route table the CLI prints (`renderRoutes`).

use super::Route;

/// One row per route, then the no-capacity and Jev notes.
pub(crate) fn render_routes(routes: &[Route], note: Option<&str>) -> String {
  let mut lines = vec![format!(
    "{:<28} {:<9} {:>5} {:<9} {:<28} effort  source",
    "issue", "tier", "score", "host", "model"
  )];
  for route in routes {
    let score = route
      .score
      .map_or_else(|| "-".to_owned(), |score| format!("{score:.2}"));
    lines.push(format!(
      "{:<28} {:<9} {score:>5} {:<9} {:<28} {:<7} {}",
      route.reference,
      route.tier,
      route.host.as_deref().unwrap_or("NONE"),
      route.model.as_deref().unwrap_or("(host default)"),
      route.effort.as_deref().unwrap_or("-"),
      route.source
    ));
  }
  if routes.iter().any(|route| route.host.is_none()) {
    lines.push("NONE = no host has free capacity; wait for a slot.".to_owned());
  }
  if let Some(note) = note {
    lines.push(note.to_owned());
  }
  lines.join("\n")
}

#[cfg(test)]
#[path = "tests/render_test.rs"]
mod tests;
