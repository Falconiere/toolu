//! Real-binary `toolu epic token url` against a live engine, and a token
//! rotated while that engine runs: the new token controls it, the old one
//! does not, and the engine prints neither.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn toolu(root: &Path) -> Command {
  let mut command = Command::new(TOOLU);
  command
    .env_clear()
    .env("PATH", "/usr/bin:/bin")
    .env("HOME", root)
    .env("TOOLU_RESOURCE_HOME", root.join("resources"))
    .env("TOOLU_EPIC_HERDR", "off")
    .arg("--config-dir")
    .arg(root.join("config"));
  // Coverage runs need the profile path, or default_*.profraw lands in the crate.
  if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  command
}

fn run(root: &Path, args: &[&str]) -> Res<Output> {
  Ok(toolu(root).args(args).output()?)
}

fn engine(root: &Path) -> Res<Child> {
  let child = toolu(root)
    .args(["epic", "engine", "--http", "127.0.0.1:0"])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let http = root.join("resources/http.json");
  let start = Instant::now();
  while !http.is_file() {
    if start.elapsed() > Duration::from_secs(10) {
      return Err("the engine never wrote http.json".into());
    }
    thread::sleep(Duration::from_millis(20));
  }
  Ok(child)
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

/// The status code of `POST /api/v1/actions/pause` with `token`.
fn pause(url: &str, token: &str) -> Res<u16> {
  let host = url
    .trim_start_matches("http://")
    .split('/')
    .next()
    .ok_or("no host")?;
  let mut stream = TcpStream::connect(host)?;
  let request = format!(
    "POST /api/v1/actions/pause HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\nOrigin: http://{host}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
  );
  stream.write_all(request.as_bytes())?;
  let mut response = String::new();
  stream.read_to_string(&mut response)?;
  Ok(response.get(9..12).ok_or("no status")?.parse()?)
}

fn secrets_file(root: &Path) -> PathBuf {
  root.join("config/toolu/secrets.json")
}

/// Rotate the token and return the new dashboard URL and its token, once
/// the listener's one-second re-read window has passed.
fn rotate(root: &Path) -> Res<(String, String)> {
  if !run(root, &["epic", "token", "new"])?.status.success() {
    return Err("token new failed".into());
  }
  let url = stdout(&run(root, &["epic", "token", "url"])?);
  let token = url.split_once("#token=").ok_or("no fragment")?.1.to_owned();
  if !std::fs::read_to_string(secrets_file(root))?.contains(&token) {
    return Err("the URL token is not the stored token".into());
  }
  thread::sleep(Duration::from_millis(1_100));
  Ok((url, token))
}

/// Everything the engine printed, and every journal file.
fn engine_output_and_journal(child: Child, root: &Path) -> Res<(String, String)> {
  let output = child.wait_with_output()?;
  let printed = format!("{}{}", stdout(&output), stderr(&output));
  let mut journal = String::new();
  for file in std::fs::read_dir(root.join("resources/journal"))? {
    journal.push_str(&std::fs::read_to_string(file?.path())?);
  }
  Ok((printed, journal))
}

#[test]
fn token_url_names_why_not_then_prints_a_url_whose_token_controls_the_engine() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  let none = run(root, &["epic", "token", "url"]).unwrap();
  assert_eq!(none.status.code(), Some(1));
  assert!(stderr(&none).contains("no engine is running"), "{none:?}");

  let mut child = engine(root).unwrap();
  let missing = run(root, &["epic", "token", "url"]).unwrap();
  assert_eq!(missing.status.code(), Some(1));
  assert!(stderr(&missing).contains("no status token"), "{missing:?}");

  let (first, old) = rotate(root).unwrap();
  assert!(first.starts_with("http://127.0.0.1:"), "{first}");
  assert_eq!(pause(&first, &old).unwrap(), 200);
  let (second, new) = rotate(root).unwrap();
  assert_ne!(old, new);
  assert_eq!(pause(&second, &old).unwrap(), 401);
  assert_eq!(pause(&second, &new).unwrap(), 200);

  child.kill().unwrap();
  let (printed, journal) = engine_output_and_journal(child, root).unwrap();
  assert!(journal.contains("all by=web"), "{journal}");
  for token in [&old, &new] {
    assert!(!printed.contains(token.as_str()) && !journal.contains(token.as_str()));
  }
}
