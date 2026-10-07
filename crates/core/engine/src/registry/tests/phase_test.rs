use toolu_runtime::registry::{ModuleKind, RegistryEvent};

use super::{registry_event, step_kind};
use crate::dispatch::Phase;
use crate::trace::StepKind;

#[test]
fn each_phase_reads_its_directory_and_each_file_kind_its_step() {
  assert_eq!(registry_event(Phase::Pre), RegistryEvent::ToolPre);
  assert_eq!(registry_event(Phase::Post), RegistryEvent::ToolPost);
  assert_eq!(step_kind(ModuleKind::Esm), StepKind::Esm);
  assert_eq!(step_kind(ModuleKind::Bash), StepKind::Executable);
  assert_eq!(step_kind(ModuleKind::Manifest), StepKind::Rule);
}
