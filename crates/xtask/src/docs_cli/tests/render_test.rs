use serde_json::{Value, json};

use super::{MARKER, pages};

/// A tree with the shapes `toolu commands --json` produces: a live command, a
/// planned namespace with its hidden hook verb and alias, and a built-in.
fn tree() -> Value {
  json!({
    "commands": [
      { "name": "hook", "path": ["hook"], "about": "Run a hook entry", "owner": "toolu",
        "aliases": [], "hidden": false, "placeholder": false, "commands": [] },
      { "name": "epic", "path": ["epic"], "about": "Drive an epic", "owner": "epic-orchestrator",
        "aliases": ["epic-orchestrator"], "hidden": false, "placeholder": false, "commands": [
          { "name": "planned", "path": ["epic", "planned"], "about": "Not ported yet",
            "hidden": false, "placeholder": true, "commands": [] },
          { "name": "hook", "path": ["epic", "hook"], "about": "Run a hook entry",
            "hidden": true, "placeholder": false, "commands": [] },
        ] },
      { "name": "secret", "path": ["secret"], "about": "Hidden", "owner": "toolu",
        "aliases": [], "hidden": true, "placeholder": false, "commands": [] },
      { "name": "commands", "path": ["commands"], "about": "List every command",
        "owner": "toolu-cli", "aliases": [], "hidden": false, "placeholder": false,
        "commands": [] },
    ]
  })
}

/// `toolu <args>`: help text with trailing spaces, as clap prints a blank line.
fn help(args: &[&str]) -> Result<String, String> {
  let path = args.strip_suffix(&["--help"]).ok_or("not a help call")?;
  Ok(format!(
    "Usage: toolu {}   \n   \nOptions:\n",
    path.join(" ")
  ))
}

#[test]
fn the_index_lists_visible_commands_with_owner_and_status() {
  let files = pages(&tree(), &help).unwrap();
  let names: Vec<&str> = files.keys().map(String::as_str).collect();
  assert_eq!(names, ["README.md", "commands.md", "epic.md", "hook.md"]);
  let readme = &files["README.md"];
  assert!(readme.starts_with(&format!("{MARKER}\n\n# `toolu` CLI reference")));
  assert!(
    readme.contains("| [`hook`](hook.md) | `plugins/toolu` | available | Run a hook entry |")
  );
  assert!(
    readme
      .contains("| [`epic`](epic.md) | `plugins/epic-orchestrator` | planned | Drive an epic |")
  );
  assert!(readme.contains("| [`commands`](commands.md) | built into `crates/cli` | available |"));
  assert!(!readme.contains("secret"));
  assert!(
    readme
      .contains("The placeholder `planned` verbs are exempt: they go when the real verbs land.")
  );
  assert!(
    readme.ends_with("```text\nUsage: toolu\n\nOptions:\n```\n"),
    "{readme}"
  );
}

#[test]
fn a_page_holds_the_help_of_each_visible_command_beneath_it() {
  let files = pages(&tree(), &help).unwrap();
  let epic = &files["epic.md"];
  assert!(epic.starts_with(&format!("{MARKER}\n\n# `toolu epic`\n\nDrive an epic.\n")));
  assert!(epic.contains("Owner: `plugins/epic-orchestrator`.\nAlias: `epic-orchestrator`.\n"));
  assert!(epic.contains("## `toolu epic`\n\n```text\nUsage: toolu epic\n\nOptions:\n```"));
  assert!(epic.contains("## `toolu epic planned`\n"));
  assert!(!epic.contains("toolu epic hook"));
  assert!(!files["hook.md"].contains("Alias:"));
}

#[test]
fn a_failing_help_call_fails_the_render() {
  let failing = |_: &[&str]| Err("toolu exited 2".to_owned());
  assert_eq!(pages(&tree(), &failing), Err("toolu exited 2".to_owned()));
}
