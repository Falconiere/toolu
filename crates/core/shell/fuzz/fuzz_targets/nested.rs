//! Fuzz the nesting the walk recurses through: the input bytes open, fill and
//! close quotes, substitutions, groups, heredocs and `bash -c`/`eval` strings,
//! so deep and mixed nesting is reached far more often than from random bytes.
#![no_main]

/// Constructs a byte can open, with the text that closes each.
const OPEN: [(&str, &str); 16] = [
  ("$(", ")"),
  ("`", "`"),
  ("\"", "\""),
  ("'", "'"),
  ("bash -c '", "'"),
  ("eval \"", "\""),
  ("{ ", "; }"),
  ("( ", " )"),
  ("if ", "; then :; fi"),
  ("[[ -n ", " ]]"),
  ("$((", "))"),
  ("${x:-", "}"),
  ("cat <<EOF\n", "\nEOF\n"),
  ("<(", ")"),
  ("time ", ""),
  ("for i in ", "; do :; done"),
];

/// Words and operators a byte can write.
const WORDS: [&str; 16] = [
  "git", " push", " node", " -e", " x", " | ", " && ", "; ", "\n", " > .env", " 2>&1", "\\\n",
  " $v", " *.ts", " {a,b}", " # c",
];

/// The script the bytes describe; constructs left open are closed only when the
/// last byte is even, so malformed scripts are fuzzed too.
fn script(data: &[u8]) -> String {
  let mut text = String::new();
  let mut open: Vec<&str> = Vec::new();
  for byte in data {
    let pick = usize::from(byte >> 2) % 16;
    match byte % 4 {
      0 | 1 => text.push_str(WORDS.get(pick).copied().unwrap_or("")),
      2 => {
        let (start, end) = OPEN.get(pick).copied().unwrap_or(("", ""));
        text.push_str(start);
        open.push(end);
      }
      _ => text.push_str(open.pop().unwrap_or("")),
    }
  }
  if data.last().is_some_and(|byte| byte % 2 == 0) {
    while let Some(end) = open.pop() {
      text.push_str(end);
    }
  }
  text
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
  let analysis = toolu_shell::analyze(&script(data));
  std::hint::black_box(toolu_shell::git::runs_git_subcommand(&analysis, "push"));
  std::hint::black_box(toolu_shell::writes::write_targets(&analysis).len());
});
