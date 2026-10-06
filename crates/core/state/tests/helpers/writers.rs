//! The writers of `interleave.rs`: Rust (this test binary re-executed),
//! TypeScript (`gate-writer.ts` under bun), and a lock holder of each.

use std::path::{Path, PathBuf};
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

fn repo_file(rel: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../..")
    .join(rel)
}

/// Who writes, and how.
pub(crate) enum Writer {
  Rust(String),
  TypeScript(String),
  RustHolder,
  TypeScriptHolder,
}

impl Writer {
  pub(crate) fn rust(id: &str) -> Writer {
    Writer::Rust(id.to_owned())
  }

  pub(crate) fn typescript(id: &str) -> Writer {
    Writer::TypeScript(id.to_owned())
  }

  pub(crate) fn rust_holder() -> Writer {
    Writer::RustHolder
  }

  pub(crate) fn typescript_holder() -> Writer {
    Writer::TypeScriptHolder
  }

  pub(crate) fn id(&self) -> &str {
    match self {
      Writer::Rust(id) | Writer::TypeScript(id) => id,
      Writer::RustHolder | Writer::TypeScriptHolder => "holder",
    }
  }

  /// Whether this writer is the TypeScript `gate-writer.ts`.
  pub(crate) fn is_typescript(&self) -> bool {
    matches!(self, Writer::TypeScript(_))
  }

  /// The process for `gate`: `count` records then even clears, in `mode`; a
  /// Rust writer then cycles until `<gate>.stop` exists.
  pub(crate) fn command(&self, gate: &Path, count: usize, mode: &str) -> Res<Command> {
    let me = std::env::current_exe().map_err(|err| err.to_string())?;
    let gate = gate.display().to_string();
    let mut command = match self {
      Writer::Rust(_) | Writer::RustHolder => Command::new(me),
      Writer::TypeScript(_) | Writer::TypeScriptHolder => Command::new("bun"),
    };
    match self {
      Writer::Rust(id) => {
        let stop = format!("{gate}.stop");
        command.args(["writer", &gate, id, &count.to_string(), mode, &stop])
      }
      Writer::TypeScript(id) => {
        let script = repo_file("packages/toolu-core/src/state/__tests__/gate-writer.ts");
        command
          .arg(script)
          .args([&gate, id, &count.to_string(), mode])
      }
      Writer::RustHolder => command.args(["hold", &gate]),
      Writer::TypeScriptHolder => {
        let io = repo_file("packages/toolu-core/src/state/state-io.ts")
          .display()
          .to_string();
        let script = format!(
          "import {{ withLock }} from {io:?}; withLock(process.argv[1] ?? \"\", () => Bun.sleepSync(60000));"
        );
        command.args(["-e", &script, &gate])
      }
    };
    Ok(command)
  }
}
