//! Quality-bar fixtures judged by cargo-deny, cargo-machete, jscpd, llvm-cov and
//! the xtask workspace checks, through `cargo xtask gate`.

#[path = "helpers/fixture.rs"]
mod fixture;

#[test]
fn capabilities_clean() {
  fixture::check("capabilities", "clean").unwrap();
}

#[test]
fn capabilities_violating_http() {
  fixture::check("capabilities", "violating-http").unwrap();
}

#[test]
fn coverage_clean() {
  fixture::check("coverage", "clean").unwrap();
}

#[test]
fn coverage_violating() {
  fixture::check("coverage", "violating").unwrap();
}

#[test]
fn dead_code_clean() {
  fixture::check("dead-code", "clean").unwrap();
}

#[test]
fn dead_code_violating_unused_pub() {
  fixture::check("dead-code", "violating-unused-pub").unwrap();
}

#[test]
fn dependencies_clean() {
  fixture::check("dependencies", "clean").unwrap();
}

#[test]
fn dependencies_violating() {
  fixture::check("dependencies", "violating").unwrap();
}

#[test]
fn dependencies_violating_anyhow() {
  fixture::check("dependencies", "violating-anyhow").unwrap();
}

#[test]
fn dependencies_violating_git_source() {
  fixture::check("dependencies", "violating-git-source").unwrap();
}

#[test]
fn dependencies_violating_license() {
  fixture::check("dependencies", "violating-license").unwrap();
}

#[test]
fn dependencies_violating_native_tls() {
  fixture::check("dependencies", "violating-native-tls").unwrap();
}

#[test]
fn dependencies_violating_openssl() {
  fixture::check("dependencies", "violating-openssl").unwrap();
}

#[test]
fn dependencies_violating_unused() {
  fixture::check("dependencies", "violating-unused").unwrap();
}

#[test]
fn dependencies_violating_versions() {
  fixture::check("dependencies", "violating-versions").unwrap();
}

#[test]
fn dependencies_violating_wildcard() {
  fixture::check("dependencies", "violating-wildcard").unwrap();
}

#[test]
fn duplication_clean() {
  fixture::check("duplication", "clean").unwrap();
}

#[test]
fn duplication_violating() {
  fixture::check("duplication", "violating").unwrap();
}

#[test]
fn fuzz_clean() {
  fixture::check("fuzz", "clean").unwrap();
}

#[test]
fn layers_clean() {
  fixture::check("layers", "clean").unwrap();
}

#[test]
fn layers_violating() {
  fixture::check("layers", "violating").unwrap();
}

#[test]
fn real_tests_clean() {
  fixture::check("real-tests", "clean").unwrap();
}

#[test]
fn real_tests_violating() {
  fixture::check("real-tests", "violating").unwrap();
}
