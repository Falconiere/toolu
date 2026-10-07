//! Machine-sample parsers (`pressure-sample.test.ts`) and a live sample of this machine.

use super::{
  meminfo_bytes, parse_load, parse_proc_stat, sample_resources, steal_since,
  vm_stat_available_bytes,
};
use crate::resources::pressure::{Ticks, sample_of};

const VM_STAT: &str = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
Pages free:                               10.\n\
Pages active:                             99.\n\
Pages inactive:                           20.\n\
Pages speculative:                         5.\n\
Pages purgeable:                          77.\n";

#[test]
fn vm_stat_counts_free_inactive_and_speculative_pages() {
  assert_eq!(vm_stat_available_bytes(VM_STAT), Some(35.0 * 16384.0));
  let missing = VM_STAT.replace("Pages speculative:                         5.\n", "");
  assert_eq!(vm_stat_available_bytes(&missing), None);
  assert_eq!(vm_stat_available_bytes("garbage"), None);
}

#[test]
fn proc_files_give_cpus_ticks_memory_and_load() {
  let stat =
    "cpu  10 1 5 100 2 0 1 7 3 0\ncpu0 5 0 2 50 1 0 0 3 0 0\ncpu1 5 1 3 50 1 0 1 4 3 0\nintr 1\n";
  let (cpus, ticks) = parse_proc_stat(stat);
  assert_eq!(cpus, 2);
  assert_eq!(
    ticks,
    Some(Ticks {
      total: 126.0,
      steal: 7.0
    })
  );
  let meminfo = "MemTotal:       32000 kB\nMemFree:         1000 kB\nMemAvailable:   16000 kB\n";
  assert_eq!(meminfo_bytes(meminfo, "MemTotal"), Some(32000.0 * 1024.0));
  assert_eq!(
    meminfo_bytes(meminfo, "MemAvailable"),
    Some(16000.0 * 1024.0)
  );
  assert_eq!(meminfo_bytes(meminfo, "Missing"), None);
  assert_eq!(meminfo_bytes("MemTotal: x kB\n", "MemTotal"), None);
  assert!((parse_load("1.25 0.5 0.1 1/2 3\n") - 1.25).abs() < f64::EPSILON);
  assert!((parse_load("{ 2.50 1.0 0.5 }") - 2.5).abs() < f64::EPSILON);
  assert!(parse_load("").abs() < f64::EPSILON);
}

#[test]
fn steal_is_the_share_of_new_ticks() {
  let before = Ticks {
    total: 100.0,
    steal: 10.0,
  };
  assert_eq!(
    steal_since(
      Some(before),
      Ticks {
        total: 200.0,
        steal: 35.0
      }
    ),
    Some(0.25)
  );
  assert_eq!(
    steal_since(
      Some(before),
      Ticks {
        total: 100.0,
        steal: 50.0
      }
    ),
    None
  );
  assert_eq!(
    steal_since(
      Some(before),
      Ticks {
        total: 200.0,
        steal: 0.0
      }
    ),
    Some(0.0)
  );
  assert_eq!(steal_since(None, before), None);
}

#[test]
fn this_machine_yields_a_valid_sample() {
  let sample = sample_resources(None, 1_000.0);
  assert!(sample_of(&sample.to_ordered()).is_some(), "{sample:?}");
  let next = sample_resources(Some(&sample), 2_000.0);
  assert!(sample_of(&next.to_ordered()).is_some(), "{next:?}");
}
