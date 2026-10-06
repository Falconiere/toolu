//! The `diff-sha` cases of `fixtures/state/cases.json` (AC-4, AC-11): real
//! repositories built by each case's steps, hashed as `diff-sha.test.ts`
//! checks them for TypeScript.

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::path::Path;

use cases::{cases_of, field, text};
use sandbox::{Res, Sandbox};
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;

fn strings(value: &Ordered) -> Vec<String> {
  let Ordered::Array(items) = value else {
    return Vec::new();
  };
  let texts = items.iter().map(|item| {
    if let Ordered::String(text) = item {
      Some(text.clone())
    } else {
      None
    }
  });
  texts.flatten().collect()
}

/// `{ "$template" | "$path": … }` with `$ROOT`, `$PROJECT` and `$HOME` filled in.
fn materialize(sb: &Sandbox, value: &Ordered) -> Res<String> {
  let raw = match value {
    Ordered::String(text) => return Ok(text.clone()),
    Ordered::Object(_) => text(value, "$template").or_else(|_| text(value, "$path"))?,
    Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) | Ordered::Array(_) => {
      return Err("not a string or a tagged value".to_owned());
    }
  };
  let root = sb
    .project
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_default();
  let pairs = [
    ("$PROJECT", &sb.project),
    ("$HOME", &sb.home),
    ("$ROOT", &root),
  ];
  Ok(pairs.iter().fold(raw, |raw, (token, path)| {
    raw.replace(token, &path.display().to_string())
  }))
}

/// The case's repository: `files` committed on `main` when `git`, then its steps.
fn diff_repo(case: &Ordered) -> Res<Sandbox> {
  let sb = Sandbox::new(None)?;
  let Ordered::Object(files) = field(case, "files")? else {
    return Err("files".to_owned());
  };
  for (path, body) in files {
    let Ordered::String(body) = body else {
      return Err("body".to_owned());
    };
    std::fs::write(sb.project.join(path), body).map_err(|err| err.to_string())?;
  }
  if field(case, "git")? == &Ordered::Bool(true) {
    sb.git(&["init", "-q", "-b", "main"])?;
    sb.git(&["add", "-A"])?;
    sb.git(&[
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "harness: initial commit",
    ])?;
  }
  let Ordered::Array(steps) = field(case, "steps")? else {
    return Err("steps".to_owned());
  };
  for step in steps {
    apply_step(&sb, step)?;
  }
  Ok(sb)
}

/// One `git`, `write` or `repeat-write` step.
fn apply_step(sb: &Sandbox, step: &Ordered) -> Res<()> {
  let op = text(step, "op")?;
  if op == "git" {
    let args = strings(field(step, "args")?);
    sb.git(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    return Ok(());
  }
  let body = text(step, "body")?;
  let body = if op == "repeat-write" {
    let Ordered::Number(count) = field(step, "count")? else {
      return Err("count".to_owned());
    };
    body.repeat(usize::try_from(count.as_u64().ok_or("count")?).map_err(|err| err.to_string())?)
  } else {
    body
  };
  std::fs::write(sb.project.join(text(step, "path")?), body).map_err(|err| err.to_string())
}

#[test]
fn diff_hashes_match_the_typescript_cases() {
  let cases = cases_of("state/cases.json", "diff-sha").unwrap();
  assert_eq!(cases.len(), 6);
  for case in &cases {
    let name = text(case, "name").unwrap();
    let sb = diff_repo(case).unwrap();
    let base = materialize(&sb, field(case, "base").unwrap()).unwrap();
    let sha = diff_sha(&sb.env(), &sb.project, &base);
    match text(case, "expect").unwrap().as_str() {
      "undefined" => assert_eq!(sha, None, "{name}"),
      "empty-blob" => assert_eq!(sha, Some(text(case, "hash").unwrap()), "{name}"),
      _ => {
        let sha = sha.unwrap();
        assert!(
          sha.len() >= 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
          "{name}"
        );
        assert_ne!(Some(sha), text(case, "notHash").ok(), "{name}");
      }
    }
    if let Some(planted) = case.get("planted") {
      assert!(
        !std::path::Path::new(&materialize(&sb, planted).unwrap()).exists(),
        "{name}"
      );
    }
  }
}
