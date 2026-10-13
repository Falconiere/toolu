use super::apply;
use crate::opencode::state::{Plan, Write, load};
use crate::step::{Status, Step};
use crate::support::{Sandbox, global};

fn plan(sb: &Sandbox, text: &str) -> Plan {
  Plan {
    steps: vec![Step::new(
      "@toolu/opencode",
      Status::Installed,
      "added",
      Vec::new(),
    )],
    writes: vec![Write {
      path: sb.path(&global("opencode.json")),
      text: text.to_owned(),
    }],
  }
}

#[test]
fn a_dry_run_writes_nothing_and_a_real_run_writes_every_file() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  let state = load(&project, &sb.env()).expect("state");
  let steps = apply(&state, plan(&sb, "{}\n"), true).expect("dry");
  assert_eq!(steps.len(), 1);
  assert!(!sb.path(&global("opencode.json")).exists());
  apply(&state, plan(&sb, "{}\n"), false).expect("write");
  assert_eq!(sb.read(&global("opencode.json")), "{}\n");
}

#[test]
fn a_file_changed_since_the_read_stops_every_write() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  sb.write(&global("opencode.json"), "{ \"plugin\": [] }\n");
  let state = load(&project, &sb.env()).expect("state");
  sb.write(
    &global("opencode.json"),
    "{ \"share\": \"edited meanwhile\" }\n",
  );
  let Err(fail) = apply(&state, plan(&sb, "{ \"plugin\": [\"x\"] }\n"), false) else {
    panic!("a concurrent edit was overwritten");
  };
  assert_eq!(fail.exit.code(), 1);
  assert!(
    fail
      .message
      .ends_with("changed while toolu ran; nothing was written"),
    "{}",
    fail.message
  );
  assert_eq!(
    sb.read(&global("opencode.json")),
    "{ \"share\": \"edited meanwhile\" }\n"
  );
  let blocked = Sandbox::new();
  let project = blocked.git_project("project");
  let state = load(&project, &blocked.env()).expect("state");
  blocked.write("not-a-dir", "a file where a directory goes");
  let unwritable = Plan {
    steps: Vec::new(),
    writes: vec![Write {
      path: blocked.path("not-a-dir/opencode.json"),
      text: "{}".to_owned(),
    }],
  };
  let failed = apply(&state, unwritable, false).map_err(|fail| fail.message);
  assert!(failed.is_err_and(|message| message.starts_with("cannot write ")));
}

#[test]
fn one_changed_file_stops_the_writes_of_the_others() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  sb.write(&global("opencode.json"), "{ \"plugin\": [] }\n");
  sb.write(
    &global("toolu/plugins.json"),
    "{\n  \"version\": 1,\n  \"enabled\": []\n}\n",
  );
  let state = load(&project, &sb.env()).expect("state");
  sb.write(
    &global("toolu/plugins.json"),
    "{\n  \"version\": 1,\n  \"enabled\": [\"jev\"]\n}\n",
  );
  let both = Plan {
    steps: Vec::new(),
    writes: vec![
      Write {
        path: sb.path(&global("opencode.json")),
        text: "{ \"plugin\": [\"x\"] }\n".to_owned(),
      },
      Write {
        path: sb.path(&global("toolu/plugins.json")),
        text: "{}".to_owned(),
      },
    ],
  };
  assert!(
    apply(&state, both, false).is_err_and(|fail| fail.message.contains("changed while toolu ran"))
  );
  assert_eq!(sb.read(&global("opencode.json")), "{ \"plugin\": [] }\n");
}

#[test]
fn a_config_edit_must_stay_jsonc_and_change_something() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  sb.write(&global("opencode.json"), "{ \"plugin\": [] }\n");
  let state = load(&project, &sb.env()).expect("state");
  let same = plan(&sb, "{ \"plugin\": [] }\n");
  assert!(apply(&state, same, false).is_err_and(|fail| fail.message.contains("changed nothing")));
  let broken = plan(&sb, "{ \"plugin\": [");
  assert!(
    apply(&state, broken, false).is_err_and(|fail| fail.message.contains("is not valid JSONC"))
  );
  assert_eq!(sb.read(&global("opencode.json")), "{ \"plugin\": [] }\n");
}

#[test]
fn a_rename_that_fails_names_what_was_written_and_leaves_no_temporary_file() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  let state = load(&project, &sb.env()).expect("state");
  std::fs::create_dir_all(sb.path("out/busy/inner")).expect("busy");
  let plan = Plan {
    steps: Vec::new(),
    writes: vec![
      Write {
        path: sb.path("out/first.json"),
        text: "{}".to_owned(),
      },
      Write {
        path: sb.path("out/busy"),
        text: "{}".to_owned(),
      },
      Write {
        path: sb.path("out/third.json"),
        text: "{}".to_owned(),
      },
    ],
  };
  let Err(fail) = apply(&state, plan, false) else {
    panic!("a rename onto a directory succeeded");
  };
  let first = sb.path("out/first.json").display().to_string();
  assert!(
    fail
      .message
      .ends_with(&format!("; already written: {first}")),
    "{}",
    fail.message
  );
  assert!(!sb.path("out/third.json").exists());
  let temps = std::fs::read_dir(sb.path("out"))
    .expect("out")
    .flatten()
    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
    .count();
  assert_eq!(temps, 0);
}
