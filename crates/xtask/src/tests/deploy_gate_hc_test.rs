use super::DOCKERFILE;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Real hc integration tests need herdr-core at tests.yml's pin, here and in release-native.
#[test]
fn the_image_and_release_gate_pin_tests_yml_herdr_core() {
  let workflow =
    std::fs::read_to_string(format!("{ROOT}/.github/workflows/release-native.yml")).unwrap();
  let tests = std::fs::read_to_string(format!("{ROOT}/.github/workflows/tests.yml")).unwrap();
  let pins: Vec<&str> = tests
    .lines()
    .filter(|line| line.trim().starts_with("Linux:"))
    .filter_map(|line| {
      line
        .split_once(") ")
        .map(|(_, pin)| pin.trim_end_matches(" ;;"))
    })
    .collect();
  assert_eq!(pins.len(), 2, "{pins:?}");
  for pin in pins {
    assert!(DOCKERFILE.contains(pin), "the image lacks hc {pin}");
  }
  let (_, url) = tests
    .split_once("https://releases.herdr.dev/core/")
    .unwrap();
  let release = url.split('/').next().unwrap();
  for file in [DOCKERFILE, workflow.as_str()] {
    assert!(
      file.contains(&format!("herdr.dev/core/{release}/")),
      "hc is not {release}"
    );
  }
  let (_, x86) = tests.split_once("target=linux-x86_64; sha=").unwrap();
  let sha = x86.split(' ').next().unwrap();
  assert!(
    workflow.contains(&format!("sha={sha}\n")),
    "release-native lacks the hc pin"
  );
}
