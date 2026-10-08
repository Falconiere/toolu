use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use toolu_http_test_support::Fixture;

use crate::{Error, pem_file_to_der, pem_to_der};

fn pem(der: &[u8], label: &str) -> String {
  let body = STANDARD.encode(der);
  let lines: Vec<&str> = body
    .as_bytes()
    .chunks(64)
    .filter_map(|chunk| std::str::from_utf8(chunk).ok())
    .collect();
  format!(
    "-----BEGIN {label}-----\n{}\n-----END {label}-----\n",
    lines.join("\n")
  )
}

#[test]
fn a_certificate_section_becomes_its_der() {
  let fixture = Fixture::start().unwrap();
  let der = fixture.root_ca_der();
  let text = pem(der, "CERTIFICATE");
  assert_eq!(pem_to_der(text.as_bytes()).unwrap(), der);
  // The first certificate wins, and other sections before it are skipped.
  let bundle = format!("{}{text}{text}", pem(&[1, 2, 3], "PRIVATE KEY"));
  assert_eq!(pem_to_der(bundle.as_bytes()).unwrap(), der);
}

#[test]
fn no_certificate_or_an_invalid_one_is_an_error() {
  assert!(matches!(pem_to_der(b""), Err(Error::InvalidConfig(_))));
  assert!(matches!(
    pem_to_der(b"not pem"),
    Err(Error::InvalidConfig(_))
  ));
  let key = pem(&[1, 2, 3], "PRIVATE KEY");
  assert!(matches!(
    pem_to_der(key.as_bytes()),
    Err(Error::InvalidConfig(_))
  ));
  let junk = pem(&[0, 1, 2], "CERTIFICATE");
  assert!(matches!(
    pem_to_der(junk.as_bytes()),
    Err(Error::InvalidConfig(_))
  ));
}

#[test]
fn a_file_is_read_then_parsed_and_a_missing_one_is_an_error() {
  let fixture = Fixture::start().unwrap();
  let dir = std::env::temp_dir().join(format!("toolu-http-pem-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let file = dir.join("ca.pem");
  std::fs::write(&file, pem(fixture.root_ca_der(), "CERTIFICATE")).unwrap();
  let path = file.to_str().unwrap();
  assert_eq!(pem_file_to_der(path).unwrap(), fixture.root_ca_der());
  let missing = dir.join("absent.pem");
  assert!(matches!(
    pem_file_to_der(missing.to_str().unwrap()),
    Err(Error::InvalidConfig(_))
  ));
  std::fs::remove_dir_all(&dir).unwrap();
}
