use super::{same_platform, supports};

#[test]
fn reporting_requires_matching_version_and_all_binding_flags() {
  let version = format!("toolu {}\n", env!("CARGO_PKG_VERSION"));
  let help = b"--file <FILE> --attempt <ID> --base <SHA> --status <STATUS> --commit <SHA>";
  assert!(supports(version.as_bytes(), help));
  assert!(!supports(b"toolu 0.0.0", help));
  assert!(!supports(
    version.as_bytes(),
    b"--file <FILE> --attempt <ID>"
  ));
}

#[test]
fn staging_requires_matching_platform_bytes() {
  assert!(same_platform(b"Darwin arm64\n", b"Darwin arm64\n"));
  assert!(!same_platform(b"Darwin arm64\n", b"Linux aarch64\n"));
}
