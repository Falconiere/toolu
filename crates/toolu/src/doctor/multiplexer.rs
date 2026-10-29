//! Report the selected backend without silently falling back from a pinned choice.

use super::checks::{Check, Status};
use serde_json::json;
use toolu_runtime::env::Env;

pub(super) fn check(env: &Env) -> Check {
  match toolu_mux::backend::configured(env) {
    Ok(backend) => {
      let ready = toolu_mux::backend::available(backend, env);
      Check::new(
        "multiplexer",
        if ready { Status::Ok } else { Status::Warn },
        format!(
          "{}: {}",
          backend.name(),
          if ready { "available" } else { "unavailable" }
        ),
        (!ready).then(|| {
          format!(
            "make the selected {} backend available before dispatch",
            backend.name()
          )
        }),
        json!({"backend":backend.name(),"available":ready}),
      )
    }
    Err(error) => Check::new("multiplexer", Status::Fail, error, None, json!({})),
  }
}

#[cfg(test)]
#[path = "tests/multiplexer_test.rs"]
mod tests;
