//! `cargo xtask check-gate-change` on real two-commit fixture repositories: gate
//! data ships alone, registration data may ship with the code it registers.

#[path = "helpers/fixture.rs"]
mod fixture;

#[test]
fn gate_change_clean_floor_gates_title() {
  fixture::check("gate-change", "clean-floor-gates-title").unwrap();
}

#[test]
fn gate_change_clean_registration() {
  fixture::check("gate-change", "clean-registration").unwrap();
}

#[test]
fn gate_change_violating_floor() {
  fixture::check("gate-change", "violating-floor").unwrap();
}

#[test]
fn gate_change_violating_floor_title() {
  fixture::check("gate-change", "violating-floor-title").unwrap();
}

#[test]
fn gate_change_violating_limit() {
  fixture::check("gate-change", "violating-limit").unwrap();
}
