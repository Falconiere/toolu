//! The writers of `interleave.rs`: this test binary re-executed as a writer or
//! as a lock holder. #440 retired the TypeScript writer.

use std::path::Path;
use std::process::Command;

use toolu_runtime::json::ordered::Ordered;

use crate::cases::field;
use crate::sandbox::Res;

/// `case[key]` as a whole number.
pub(crate) fn number(case: &Ordered, key: &str) -> Res<usize> {
  let Ordered::Number(number) = field(case, key)? else {
    return Err(format!("{key}: not a number"));
  };
  let whole = number.as_u64().ok_or(format!("{key}: not whole"))?;
  usize::try_from(whole).map_err(|err| err.to_string())
}

/// Who writes, and how.
pub(crate) enum Writer {
  Rust(String),
  RustHolder,
}

impl Writer {
  pub(crate) fn rust(id: &str) -> Writer {
    Writer::Rust(id.to_owned())
  }

  pub(crate) fn rust_holder() -> Writer {
    Writer::RustHolder
  }

  pub(crate) fn id(&self) -> &str {
    match self {
      Writer::Rust(id) => id,
      Writer::RustHolder => "holder",
    }
  }

  /// The process for `gate`: `count` records then even clears, in `mode`; a
  /// Rust writer then cycles until `<gate>.stop` exists.
  pub(crate) fn command(&self, gate: &Path, count: usize, mode: &str) -> Res<Command> {
    let me = std::env::current_exe().map_err(|err| err.to_string())?;
    let gate = gate.display().to_string();
    let mut command = Command::new(me);
    match self {
      Writer::Rust(id) => {
        let stop = format!("{gate}.stop");
        command.args(["writer", &gate, id, &count.to_string(), mode, &stop])
      }
      Writer::RustHolder => command.args(["hold", &gate]),
    };
    Ok(command)
  }
}
