//! `shell-writes.test.ts`: a static `open(<literal>, <mode>)` reports its path
//! in every literal form; any write the scan cannot read statically is an
//! unknown target, never silence; reads stay out.

use std::time::{Duration, Instant};

use crate::writes::tests::{paths, some};

fn script(code: &str) -> Vec<Option<String>> {
  paths(&format!(
    "python3 -c {}",
    serde_json::to_string(code).unwrap()
  ))
}

#[test]
fn dd_and_python_write_targets() {
  assert_eq!(paths("dd if=/dev/zero of=.env bs=1"), some(&[".env"]));
  assert_eq!(
    paths("python3 -c \"open('ok.txt','w'); open('.env','a+')\""),
    some(&["ok.txt", ".env"])
  );
  assert_eq!(
    paths("python3 -c \"open('.env','r+').write(y)\""),
    some(&[".env"])
  );
  assert_eq!(
    paths("python -c \"print(open('.env','rb').read())\""),
    Vec::<Option<String>>::new()
  );
  assert_eq!(
    paths("python3.12 -c 'open(\".env\", \"x\")'"),
    some(&[".env"])
  );
  assert_eq!(paths("python3 script.py"), Vec::<Option<String>>::new());
  assert_eq!(
    paths("python3 -m http.server"),
    Vec::<Option<String>>::new()
  );
}

#[test]
fn every_literal_form_is_read() {
  assert_eq!(script("open(r'.env', 'w')"), some(&[".env"]));
  assert_eq!(script("open(f'.env', 'w').write(x)"), some(&[".env"]));
  assert_eq!(script("open(b'.env', mode='wb')"), some(&[".env"]));
  assert_eq!(script("open('''.env''', 'a')"), some(&[".env"]));
  assert_eq!(script("open(\"\"\"x/.env\"\"\", \"w\")"), some(&["x/.env"]));
}

#[test]
fn a_mode_that_does_not_close_at_its_first_quote_is_an_unknown_write() {
  assert_eq!(script("open('.env', 'w' if a else 'r')"), [None]);
  assert_eq!(script("open('.env', 'r' if a else 'rb')"), [None]);
  // TypeScript's regex retries every later quote; 40,000 such calls stay linear here.
  let code = "open('a',\"".repeat(40_000);
  let started = Instant::now();
  assert_eq!(super::written(&code).len(), 40_000);
  let elapsed = started.elapsed();
  assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}

#[test]
fn writes_that_cannot_be_read_statically_are_unknown() {
  for code in [
    "open('.' + 'env', 'w')",
    "f = '.env'; open(f, 'w')",
    "open(f'{d}/.env', 'w')",
    "open('.env', encoding='utf8', mode='w')",
    "open('.env', m)",
    "from pathlib import Path; Path('.env').write_text('x')",
    "from pathlib import Path; Path('.env').open('w')",
    "import shutil; shutil.copy('a', '.env')",
    "import os; os.rename('a', '.env')",
  ] {
    assert_eq!(script(code), [None], "{code}");
  }
  assert_eq!(paths("python3 -c \"$CODE\""), [None]);
  assert_eq!(script("import shutil; shutil.copyfile ('a', 'b')"), [None]);
}

#[test]
fn reads_stay_out_of_the_write_targets() {
  for code in [
    "print(open('.env').read())",
    "open('.env', 'r')",
    "open(r'.env', mode='rb')",
    "reopen('.env', 'w')",
    "os.linkat('a', 'b')",
  ] {
    assert!(script(code).is_empty(), "{code}");
  }
}

#[test]
fn a_static_heredoc_or_herestring_fed_to_python_is_its_script() {
  assert_eq!(
    paths("python3 - <<'EOF'\nopen('.env', 'w').write('x')\nEOF"),
    some(&[".env"])
  );
  assert_eq!(paths("python3 <<< \"open('.env', 'a')\""), some(&[".env"]));
  assert_eq!(
    paths("python3 - <<'EOF'\nprint(1)\nEOF"),
    Vec::<Option<String>>::new()
  );
  assert_eq!(
    paths("python3 script.py <<'EOF'\nopen('.env','w')\nEOF"),
    Vec::<Option<String>>::new()
  );
  assert_eq!(paths("python3 <<EOF\nopen('$X', 'w')\nEOF"), [None]);
}
