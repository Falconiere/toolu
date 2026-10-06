use std::path::Path;

use super::{Block, Span, files, scan};

fn repo() -> &'static Path {
  Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn shell_fences_are_kept_and_other_fences_skipped() {
  let markdown = "intro `toolu doctor` text\n```bash\ngh pr view\n```\n```text\ntoolu epik\n```\n```\nuntagged\n```\n~~~sh\nls\n~~~\n";
  let scanned = scan(markdown);
  assert_eq!(
    scanned.blocks,
    [
      Block {
        line: 3,
        text: "gh pr view\n".to_owned()
      },
      Block {
        line: 12,
        text: "ls\n".to_owned()
      },
    ]
  );
  assert_eq!(
    scanned.spans,
    [Span {
      line: 1,
      text: "toolu doctor".to_owned()
    }]
  );
}

#[test]
fn console_blocks_keep_prompt_lines_only() {
  let scanned = scan("```console\n$ toolu --version\ntoolu 7.11.0\n```\n");
  assert_eq!(scanned.blocks[0].text, "toolu --version\n\n");
}

#[test]
fn indented_fences_unterminated_fences_and_spans_inside_fences() {
  let scanned = scan("1. Run:\n   ```bash\n   git push `x`\n   ```\n```sh\nls\n`no span`\n");
  assert_eq!(scanned.blocks.len(), 2);
  assert_eq!(
    scanned.blocks[1],
    Block {
      line: 6,
      text: "ls\n`no span`\n".to_owned()
    }
  );
  assert!(scanned.spans.is_empty(), "{:?}", scanned.spans);
}

#[test]
fn double_backtick_spans_unclosed_runs_and_space_trimming() {
  let scanned = scan("a ``x ` y`` b ` toolu hook ` c `unclosed\n`` `` d ```inline``` e\n");
  let texts: Vec<&str> = scanned
    .spans
    .iter()
    .map(|span| span.text.as_str())
    .collect();
  assert_eq!(texts, ["x ` y", "toolu hook", " ", "inline"]);
  assert_eq!(scanned.spans[2].line, 2);
}

/// docs/install.md is real live documentation: its inline mentions are found.
#[test]
fn real_docs_inline_spans_are_found() {
  let text = std::fs::read_to_string(repo().join("docs/install.md")).unwrap();
  let scanned = scan(&text);
  assert!(scanned.spans.iter().any(|span| span.text == "toolu doctor"));
}

#[test]
fn the_real_file_list_covers_every_surface_and_skips_records() {
  let list = files(repo()).unwrap();
  for wanted in [
    "AGENTS.md",
    "plugins/brainstorm/skills/brainstorm/SKILL.md",
    "plugins/delivery-flow/skills/delivery-flow/SKILL.md",
    "plugins/delivery-flow/skills/delivery-flow/references/execution.md",
    "docs/install.md",
    "docs/cli/README.md",
  ] {
    assert!(list.iter().any(|file| file == wanted), "{wanted}");
  }
  assert!(
    list
      .iter()
      .any(|file| file.starts_with("plugins/") && file.contains("/commands/"))
  );
  assert!(
    list
      .iter()
      .any(|file| file.starts_with("plugins/") && file.contains("/agents/"))
  );
  assert!(
    !list
      .iter()
      .any(|file| file.starts_with("docs/toolu/") || file.starts_with("docs/releases/"))
  );
  assert!(list.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn an_empty_root_has_no_files() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(files(dir.path()).unwrap(), Vec::<String>::new());
}
