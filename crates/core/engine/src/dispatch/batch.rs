//! One multi-path edit's reusable event view and registry plan.

use std::cell::OnceCell;

use toolu_runtime::json::ordered::Ordered;
use toolu_state::edit_records::EditRecord;

use super::ModuleResult;
use super::edits::synthetic_edit;
use super::event::{EditFields, Payload, View};
use super::session::Session;
use super::walk::walk_with_view;
use crate::registry::phase::RegistryPlan;
use crate::trace::Step;

/// Reused state for the sequential path walk.
pub(super) struct Batch<'a> {
  view: View,
  registry_plan: Option<RegistryPlan<'a>>,
}

impl<'a> Batch<'a> {
  /// Normalize one synthetic edit, then reuse its view across records.
  pub(super) fn new(
    doc: &Ordered,
    first: &EditRecord,
    session: &Session<'a>,
  ) -> Result<Self, String> {
    let initial = synthetic_edit(doc, first);
    let payload = Payload {
      text: "",
      doc: Some(&initial),
      synthetic: None,
      rendered: OnceCell::new(),
      tool_name: "Edit",
      edit: Some(EditFields::of(first)),
    };
    Ok(Batch {
      view: View::of(&payload, session)?,
      registry_plan: None,
    })
  }

  /// Walk this record, rendering its full JSON only for an executable.
  pub(super) fn walk_path(
    &mut self,
    doc: &Ordered,
    record: &EditRecord,
    session: &Session<'a>,
    trace: &mut Vec<Step>,
  ) -> ModuleResult {
    self.view.set_record(record);
    let payload = Payload {
      text: "",
      doc: None,
      synthetic: Some((doc, record)),
      rendered: OnceCell::new(),
      tool_name: "Edit",
      edit: Some(EditFields::of(record)),
    };
    walk_with_view(
      &payload,
      &self.view,
      session,
      trace,
      &mut self.registry_plan,
    )
  }
}
