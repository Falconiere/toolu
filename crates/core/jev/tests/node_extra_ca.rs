//! `NODE_EXTRA_CA_CERTS`: with no test root on `Config`, the client trusts
//! only the first certificate of the PEM file it names.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use toolu_http_test_support::{Fixture, Reply};
use toolu_jev_client::{Config, DEFAULT_MODEL, Error, Jev, Question, Questions, State};
use toolu_runtime::env::Env;

const PATH: &str = "/v1/systemone";

fn write_pem(name: &str, der: &[u8]) -> std::io::Result<PathBuf> {
  let dir = std::env::temp_dir().join(format!("toolu-jev-node-ca-{}-{name}", std::process::id()));
  std::fs::create_dir_all(&dir)?;
  let body = STANDARD.encode(der);
  let lines: Vec<String> = body
    .as_bytes()
    .chunks(64)
    .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
    .collect();
  let file = dir.join("ca.pem");
  let pem = format!(
    "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
    lines.join("\n")
  );
  std::fs::write(&file, pem)?;
  Ok(file)
}

fn jev(fixture: &Fixture, ca_file: &Path) -> Result<Jev, Error> {
  let env = Env::from_pairs([
    ("TYPESAFE_API_KEY", "fixture-key".to_owned()),
    ("HTTPS_PROXY", fixture.proxy_url()),
    (
      "NODE_EXTRA_CA_CERTS",
      ca_file.to_string_lossy().into_owned(),
    ),
  ]);
  let config = Config {
    endpoint: fixture.url(PATH),
    pause: Duration::from_millis(10),
    test_root_ca_der: None,
  };
  Jev::from_env(&env, config)
}

fn ask(jev: &Jev) -> Result<(), Error> {
  let questions = Questions::single("q", Question::noul("Urgent?", None, None)?);
  jev
    .ask(&State::text("x"), DEFAULT_MODEL, &questions)
    .map(|_| ())
}

#[test]
fn the_file_is_the_only_root_the_client_trusts() {
  let body = r#"{"model":"jev-1.13.0","answers":{"q":{"type":"noul","noul":0.92}},"usage":{"input_tokens":3,"output_tokens":2}}"#;
  let fixture = Fixture::start().expect("test setup");
  fixture
    .route(PATH, Reply::new(200, body))
    .expect("test setup");
  let trusted = write_pem("trusted", fixture.root_ca_der()).expect("pem");
  assert_eq!(ask(&jev(&fixture, &trusted).expect("test setup")), Ok(()));

  let other = Fixture::start().expect("test setup");
  let untrusted = write_pem("untrusted", other.root_ca_der()).expect("pem");
  let failed = ask(&jev(&fixture, &untrusted).expect("test setup"));
  assert!(matches!(failed, Err(Error::Transport(_))), "{failed:?}");
  // Only the first run reached the route.
  assert_eq!(fixture.requests().expect("test setup").len(), 1);
}
