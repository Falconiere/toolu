//! Stack and backtrace summary with framework frames collapsed.

use toolu_runtime::env::Env;

use super::cap;

const NOISE: &[&str] = &[
  "/rustc/",
  "node:internal",
  "node:",
  "core::",
  "std::",
  "__rustc",
  "rust_begin_unwind",
  "panic_fmt",
  "panic_bounds_check",
  "FnOnce::call_once",
  "<anonymous>",
];

#[derive(Default)]
struct Frames {
  app: Vec<String>,
  noise: usize,
}

fn emit(frames: &mut Frames, symbol: &str, location: &str) {
  if NOISE
    .iter()
    .any(|part| symbol.contains(part) || location.contains(part))
  {
    frames.noise += 1;
  } else if !symbol.is_empty() && !location.is_empty() {
    frames.app.push(format!("{symbol} @ {location}"));
  } else if !location.is_empty() {
    frames.app.push(location.to_owned());
  } else if !symbol.is_empty() {
    frames.app.push(symbol.to_owned());
  }
}

fn at_line(line: &str) -> Option<&str> {
  let trimmed = line.trim_start_matches([' ', '\t']);
  trimmed
    .strip_prefix("at ")
    .or_else(|| trimmed.strip_prefix("at\t"))
}

fn indexed_symbol(line: &str) -> Option<&str> {
  let trimmed = line.trim_start_matches([' ', '\t']);
  let (number, symbol) = trimmed.split_once(':')?;
  (!number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())).then(|| symbol.trim())
}

fn js_frame(rest: &str) -> (&str, &str) {
  let trimmed = rest.trim();
  if let Some(without_close) = trimmed.strip_suffix(')')
    && let Some((symbol, location)) = without_close.rsplit_once('(')
  {
    return (symbol.trim(), location.trim_end());
  }
  ("", trimmed)
}

fn parse(lines: &[&str]) -> Frames {
  let mut frames = Frames::default();
  let mut pending: Option<&str> = None;
  for line in lines {
    let at = at_line(line);
    if let (Some(symbol), Some(location)) = (pending, at) {
      emit(&mut frames, symbol, location.trim());
      pending = None;
      continue;
    }
    if let Some(symbol) = pending.take() {
      emit(&mut frames, symbol, "");
    }
    if let Some(rest) = at {
      let (symbol, location) = js_frame(rest);
      if !symbol.is_empty() || !location.is_empty() {
        emit(&mut frames, symbol, location);
        continue;
      }
    }
    pending = indexed_symbol(line);
  }
  if let Some(symbol) = pending {
    emit(&mut frames, symbol, "");
  }
  frames
}

fn raw(lines: &[&str], cap: usize) -> String {
  let mut out = vec!["debug-stack: no recognizable stack frames — raw input (capped):".to_owned()];
  out.extend(lines.iter().take(cap).map(|line| (*line).to_owned()));
  if lines.len() > cap {
    out.push(format!("... (+{} more lines)", lines.len() - cap));
  }
  out.join("\n")
}

/// Summarize app frames in bounded text or named JSON fields.
pub(super) fn summarize(lines: &[&str], json: bool, env: &Env) -> String {
  let frames = parse(lines);
  let recognized = !frames.app.is_empty() || frames.noise > 0;
  let max_frames = cap(env, "DEBUG_MAX_FRAMES", 20);
  if json {
    return serde_json::json!({
      "app_frames": frames.app.iter().take(max_frames).collect::<Vec<_>>(),
      "noise_frames": frames.noise,
      "recognized": recognized
    })
    .to_string();
  }
  if !recognized {
    return raw(lines, cap(env, "DEBUG_MAX_LINES", 100));
  }
  let over = frames.app.len() > max_frames;
  let count = if frames.app.is_empty() {
    String::new()
  } else {
    frames.app.len().to_string()
  };
  let mut out = vec![format!(
    "APP FRAMES ({count}{}):",
    if over { "+" } else { "" }
  )];
  out.extend(
    frames
      .app
      .iter()
      .take(max_frames)
      .map(|frame| format!("  - {frame}")),
  );
  if over {
    out.push(format!("  ... (+{} more)", frames.app.len() - max_frames));
  }
  if frames.noise > 0 {
    out.push(format!(
      "(+{} framework/runtime frames collapsed)",
      frames.noise
    ));
  }
  out.join("\n")
}

#[cfg(test)]
#[path = "../tests/stack_test.rs"]
mod tests;
