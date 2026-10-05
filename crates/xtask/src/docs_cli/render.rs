//! The Markdown pages of `docs/cli/`: an index, then one page per visible
//! top-level command holding the real `--help` output of every visible command
//! beneath it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;

use super::MARKER;

/// Runs `toolu` with the given arguments and returns its stdout.
pub(super) type Toolu<'a> = &'a dyn Fn(&[&str]) -> Result<String, String>;

/// The owner of the commands `crates/cli` provides itself.
const BUILTIN: &str = "toolu-cli";

const INTRO: &str = "Generated from the `toolu` binary by `cargo xtask docs-cli`; do not \
edit. `toolu <command> --help` is the reference: each page holds that output for every command \
beneath it, and the gate's `docs-cli` step fails when these files are stale.

- [`commands.json`](commands.json) is `toolu commands --json`, the command tree \
(`toolu.commands/v1`) that tools such as the Markdown drift gate (#444) read.
- [`commands.schema.json`](commands.schema.json) is `toolu commands --schema`, the JSON Schema of \
that tree and of every other `--json` document.
- [`installer.md`](installer.md) documents the Node installer, `npx @toolu/plugins`, until \
`toolu plugins` (#438) replaces it.

Data goes to stdout and diagnostics to stderr; with `--json`, stdout is exactly one JSON \
document. The exit codes are in the `toolu --help` output below. Within a major version every \
documented command, alias, flag, value and exit code keeps working: `cargo xtask \
check-cli-compat` fails a change that removes one unless `hookProtocol` is bumped in a breaking \
(`!`) pull request.";

const HOOKS: &str = "`hooks.json` calls `toolu hook <name> --event <Event> --plugin-root \
<dir>` for the toolu plugin and `toolu <plugin> hook <name> …` for every other plugin. Those \
plugin `hook` verbs are hidden from `--help`; [`hook.md`](hook.md) shows their arguments.";

/// Every Markdown page, by file name.
pub(super) fn pages(tree: &Value, toolu: Toolu<'_>) -> Result<BTreeMap<String, String>, String> {
  let commands = visible(tree);
  let mut files = BTreeMap::new();
  files.insert(
    "README.md".to_owned(),
    readme(&commands, &help(toolu, &[])?),
  );
  for command in &commands {
    files.insert(
      format!("{}.md", text(command, "name")),
      page(command, toolu)?,
    );
  }
  Ok(files)
}

fn visible(node: &Value) -> Vec<&Value> {
  node
    .get("commands")
    .and_then(Value::as_array)
    .map(|commands| {
      commands
        .iter()
        .filter(|command| command.get("hidden") != Some(&Value::Bool(true)))
        .collect()
    })
    .unwrap_or_default()
}

fn text<'a>(node: &'a Value, key: &str) -> &'a str {
  node.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// `toolu <path> --help`, with trailing spaces dropped from every line.
fn help(toolu: Toolu<'_>, path: &[&str]) -> Result<String, String> {
  let mut args = path.to_vec();
  args.push("--help");
  let output = toolu(&args)?;
  let lines: Vec<&str> = output.lines().map(str::trim_end).collect();
  Ok(lines.join("\n").trim_end().to_owned())
}

fn readme(commands: &[&Value], root_help: &str) -> String {
  let mut page = format!(
    "{MARKER}\n\n# `toolu` CLI reference\n\n{INTRO}\n\n## Commands\n\n\
     | Command | Owner | Status | About |\n|---|---|---|---|\n"
  );
  for command in commands {
    let name = text(command, "name");
    let status = if visible(command)
      .iter()
      .any(|child| child.get("placeholder") == Some(&Value::Bool(true)))
    {
      "planned"
    } else {
      "available"
    };
    // Writing to a String cannot fail.
    writeln!(
      page,
      "| [`{name}`]({name}.md) | {} | {status} | {} |",
      owner(text(command, "owner")),
      text(command, "about")
    )
    .ok();
  }
  write!(
    page,
    "\n## Hook entries\n\n{HOOKS}\n\n## `toolu --help`\n\n```text\n{root_help}\n```\n"
  )
  .ok();
  page
}

fn owner(owner: &str) -> String {
  if owner == BUILTIN {
    "built into `crates/cli`".to_owned()
  } else {
    format!("`plugins/{owner}`")
  }
}

fn page(command: &Value, toolu: Toolu<'_>) -> Result<String, String> {
  let name = text(command, "name");
  let mut page = format!(
    "{MARKER}\n\n# `toolu {name}`\n\n{}.\n\nOwner: {}.\n",
    text(command, "about"),
    owner(text(command, "owner"))
  );
  let aliases: Vec<String> = command
    .get("aliases")
    .and_then(Value::as_array)
    .map(|aliases| {
      aliases
        .iter()
        .filter_map(Value::as_str)
        .map(|alias| format!("`{alias}`"))
        .collect()
    })
    .unwrap_or_default();
  if !aliases.is_empty() {
    writeln!(page, "Alias: {}.", aliases.join(", ")).ok();
  }
  sections(command, &mut page, toolu)?;
  Ok(page)
}

/// A `## toolu <path>` section with its help for `node` and each visible descendant.
fn sections(node: &Value, page: &mut String, toolu: Toolu<'_>) -> Result<(), String> {
  let path: Vec<&str> = node
    .get("path")
    .and_then(Value::as_array)
    .map(|path| path.iter().filter_map(Value::as_str).collect())
    .unwrap_or_default();
  write!(
    page,
    "\n## `toolu {}`\n\n```text\n{}\n```\n",
    path.join(" "),
    help(toolu, &path)?
  )
  .ok();
  for child in visible(node) {
    sections(child, page, toolu)?;
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/render_test.rs"]
mod tests;
