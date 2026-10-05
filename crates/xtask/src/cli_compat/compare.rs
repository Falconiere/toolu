//! What a newer command tree breaks for a caller of the older one: a command,
//! alias, flag, short flag, possible value or exit code that disappeared, an
//! argument that became required, or a new required one. Placeholder verbs
//! (`planned`) are not documented verbs and may disappear.

use serde_json::Value;

/// One line per break of `before` by `after`.
pub(super) fn breaks(before: &Value, after: &Value) -> Vec<String> {
  let mut found = exit_codes(before, after);
  flags(
    "toolu",
    list(before, "flags"),
    list(after, "flags"),
    &mut found,
  );
  commands("toolu", before, after, &mut found);
  found
}

/// The findings for `breaks`, given both trees' `hookProtocol` and the PR title.
pub(super) fn verdict(
  breaks: &[String],
  before: u64,
  after: u64,
  title: Option<&str>,
) -> Vec<String> {
  if after < before {
    return vec![format!("hookProtocol decreased from {before} to {after}")];
  }
  if breaks.is_empty() {
    return Vec::new();
  }
  if after > before {
    return match title {
      Some(title) if !is_breaking(title) => vec![format!(
        "the documented CLI breaks ({}) with a hookProtocol bump, but the title `{title}` \
         lacks the conventional `!` breaking marker",
        breaks.join("; ")
      )],
      _ => Vec::new(),
    };
  }
  breaks
    .iter()
    .map(|item| {
      format!(
        "{item} — a documented command, alias, flag, value or exit code is a contract within a \
         major version: keep it, or bump HOOK_PROTOCOL in toolu-protocol and title the PR `type!: …`"
      )
    })
    .collect()
}

/// `type!:` or `type(scope)!:`.
fn is_breaking(title: &str) -> bool {
  title
    .split_once(':')
    .is_some_and(|(head, _)| head.ends_with('!') && !head.contains(' '))
}

fn list<'a>(node: &'a Value, key: &str) -> &'a [Value] {
  node
    .get(key)
    .and_then(Value::as_array)
    .map_or(&[], Vec::as_slice)
}

fn text<'a>(node: &'a Value, key: &str) -> &'a str {
  node.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn flag_set(node: &Value, key: &str) -> bool {
  node.get(key) == Some(&Value::Bool(true))
}

fn exit_codes(before: &Value, after: &Value) -> Vec<String> {
  list(before, "exitCodes")
    .iter()
    .filter(|code| {
      !list(after, "exitCodes")
        .iter()
        .any(|now| now.get("code") == code.get("code") && now.get("name") == code.get("name"))
    })
    .map(|code| {
      format!(
        "exit code {} ({}) removed or renamed",
        code.get("code").unwrap_or(&Value::Null),
        text(code, "name")
      )
    })
    .collect()
}

/// The child of `parent` that answers to `name`, by name or alias.
fn child<'a>(parent: &'a Value, name: &str) -> Option<&'a Value> {
  list(parent, "commands").iter().find(|command| {
    text(command, "name") == name
      || list(command, "aliases")
        .iter()
        .any(|alias| alias.as_str() == Some(name))
  })
}

fn commands(path: &str, before: &Value, after: &Value, found: &mut Vec<String>) {
  for old in list(before, "commands") {
    if flag_set(old, "placeholder") {
      continue;
    }
    let name = text(old, "name");
    let shown = format!("{path} {name}");
    let Some(new) = child(after, name) else {
      found.push(format!("command `{shown}` removed"));
      continue;
    };
    for alias in list(old, "aliases").iter().filter_map(Value::as_str) {
      if child(after, alias).is_none() {
        found.push(format!("alias `{alias}` of `{shown}` removed"));
      }
    }
    flags(&shown, list(old, "flags"), list(new, "flags"), found);
    positionals(&shown, list(old, "args"), list(new, "args"), found);
    commands(&shown, old, new, found);
  }
}

fn flags(path: &str, before: &[Value], after: &[Value], found: &mut Vec<String>) {
  for old in before {
    let long = text(old, "long");
    match after.iter().find(|new| text(new, "long") == long) {
      None => found.push(format!("flag `--{long}` of `{path}` removed")),
      Some(new) => flag(path, old, new, found),
    }
  }
  for new in after {
    let long = text(new, "long");
    if flag_set(new, "required") && !before.iter().any(|old| text(old, "long") == long) {
      found.push(format!("new required flag `--{long}` on `{path}`"));
    }
  }
}

fn flag(path: &str, old: &Value, new: &Value, found: &mut Vec<String>) {
  let long = text(old, "long");
  let short = old.get("short").filter(|short| !short.is_null());
  if short.is_some() && short != new.get("short") {
    found.push(format!(
      "short flag of `--{long}` on `{path}` removed or changed"
    ));
  }
  if old.get("takesValue") != new.get("takesValue") {
    found.push(format!(
      "`--{long}` on `{path}` changed whether it takes a value"
    ));
  }
  if !flag_set(old, "required") && flag_set(new, "required") {
    found.push(format!("flag `--{long}` on `{path}` became required"));
  }
  for value in list(old, "possibleValues") {
    if !list(new, "possibleValues").contains(value) {
      found.push(format!("value {value} of `--{long}` on `{path}` removed"));
    }
  }
}

fn positionals(path: &str, before: &[Value], after: &[Value], found: &mut Vec<String>) {
  for (index, old) in before.iter().enumerate() {
    match after.get(index) {
      None => found.push(format!(
        "argument {} of `{path}` removed",
        text(old, "name")
      )),
      Some(new) if !flag_set(old, "required") && flag_set(new, "required") => {
        found.push(format!(
          "argument {} of `{path}` became required",
          text(old, "name")
        ));
      }
      Some(_) => {}
    }
  }
  for new in after.iter().skip(before.len()) {
    if flag_set(new, "required") {
      found.push(format!(
        "new required argument {} on `{path}`",
        text(new, "name")
      ));
    }
  }
}

#[cfg(test)]
#[path = "tests/compare_test.rs"]
mod tests;
