//! What this machine offers: its operating system, CPU cores and memory. The
//! settings derive their defaults from these facts, which tests inject.

use toolu_runtime::process::{Spec, run};

/// The operating systems the sandbox knows apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
  /// macOS, where the sandbox is a Lima VM by default.
  Macos,
  /// Linux, where the sandbox is the host by default.
  Linux,
  /// Anything else; treated as Linux would be.
  Other,
}

impl Os {
  /// The OS named as `std::env::consts::OS` names it.
  pub(crate) fn named(name: &str) -> Os {
    match name {
      "macos" => Os::Macos,
      "linux" => Os::Linux,
      _ => Os::Other,
    }
  }
}

/// The machine's facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Facts {
  /// The operating system toolu runs on.
  pub(crate) os: Os,
  /// The CPU cores available to this process.
  pub(crate) cores: u32,
  /// Physical memory in whole GiB, rounded down.
  pub(crate) memory_gib: u32,
}

/// Bytes in one GiB.
const GIB: u64 = 1 << 30;
/// macOS's `sysctl`; `/usr/sbin` is not on every `PATH` toolu runs with.
pub(crate) const SYSCTL: &str = "/usr/sbin/sysctl";
/// What a machine that does not say is assumed to have.
const FALLBACK_CORES: u32 = 2;
const FALLBACK_MEMORY_GIB: u32 = 4;

impl Facts {
  /// This machine's facts.
  pub(crate) fn probe() -> Facts {
    let os = Os::named(std::env::consts::OS);
    let cores = std::thread::available_parallelism().map_or(FALLBACK_CORES, |count| {
      u32::try_from(count.get()).unwrap_or(u32::MAX)
    });
    let bytes = match os {
      Os::Linux => std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| meminfo_bytes(&text)),
      Os::Macos => sysctl_memsize(),
      Os::Other => None,
    };
    Facts {
      os,
      cores,
      memory_gib: bytes.map_or(FALLBACK_MEMORY_GIB, gib),
    }
  }
}

/// `MemTotal` from `/proc/meminfo`, in bytes.
pub(crate) fn meminfo_bytes(text: &str) -> Option<u64> {
  let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
  let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
  kib.checked_mul(1024)
}

/// `sysctl -n hw.memsize`, in bytes.
fn sysctl_memsize() -> Option<u64> {
  let output = run(&Spec::new([SYSCTL, "-n", "hw.memsize"])).ok()?;
  (output.exit_code == 0)
    .then(|| output.stdout.trim().parse().ok())
    .flatten()
}

fn gib(bytes: u64) -> u32 {
  u32::try_from(bytes / GIB).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "tests/facts_test.rs"]
mod tests;
