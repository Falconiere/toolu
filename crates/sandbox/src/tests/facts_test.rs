use super::{Facts, Os, gib, meminfo_bytes};

#[test]
fn the_os_is_named_as_std_names_it() {
  assert_eq!(Os::named("macos"), Os::Macos);
  assert_eq!(Os::named("linux"), Os::Linux);
  assert_eq!(Os::named("freebsd"), Os::Other);
}

#[test]
fn meminfo_gives_mem_total_in_bytes() {
  let text = "MemFree:  1000 kB\nMemTotal:       16303456 kB\nSwapTotal: 0 kB\n";
  assert_eq!(meminfo_bytes(text), Some(16_303_456 * 1024));
  assert_eq!(meminfo_bytes("MemFree: 1 kB\n"), None);
  assert_eq!(meminfo_bytes("MemTotal: lots kB\n"), None);
}

#[test]
fn bytes_round_down_to_whole_gib() {
  assert_eq!(gib(103_079_215_104), 96);
  assert_eq!(gib((1 << 30) - 1), 0);
}

#[test]
fn this_machine_has_its_os_some_cores_and_some_memory() {
  let facts = Facts::probe();
  assert_eq!(facts.os, Os::named(std::env::consts::OS));
  assert!(facts.cores >= 1, "{facts:?}");
  assert!(facts.memory_gib >= 1, "{facts:?}");
}

#[test]
#[cfg(target_os = "macos")]
fn macos_memory_is_read_with_sysctl_by_its_absolute_path() {
  use super::SYSCTL;

  assert!(std::path::Path::new(SYSCTL).is_file());
  let output = toolu_runtime::process::run(&toolu_runtime::process::Spec::new([
    SYSCTL,
    "-n",
    "hw.memsize",
  ]))
  .unwrap();
  assert_eq!(output.exit_code, 0);
  assert!(
    output.stdout.trim().parse::<u64>().is_ok(),
    "{}",
    output.stdout
  );
}
