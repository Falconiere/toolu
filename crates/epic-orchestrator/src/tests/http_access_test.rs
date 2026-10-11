use super::host_allowed;

#[test]
fn loopback_accepts_only_its_own_port_and_names() {
  let addr = "127.0.0.1:7717".parse().expect("address");
  assert!(host_allowed("localhost:7717", addr));
  assert!(host_allowed("127.0.0.1:7717", addr));
  assert!(!host_allowed("localhost:7718", addr));
  assert!(!host_allowed("foreign.example:7717", addr));
}
