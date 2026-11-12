use toolu_runtime::env::Env;

use super::{FileOutline, Item, MISSING, batches, enclosing, outline};

const RUST: &str = "/// A point. With more words.
#[derive(Debug)]
pub struct Point {
  x: i32,
}

impl Point {
  pub fn new() -> Self {
    Point { x: 0 }
  }

  fn scaled<T>(
    &self,
    by: T,
  ) -> i32
  where
    T: Into<i32>,
  {
    fn helper() {}
    self.x
  }
}

fn run() {
  fn local() {}
}
";

const TS: &str = "/**
 * Adds two numbers. Then returns.
 * @param a first
 */
export function add(
  a: number,
  b: number,
): number {
  function inner() {}
  return a + b;
}

@sealed
export class Box {
  private size(): number {
    const local = () => 1;
    return local();
  }
}
export const make = (n: number) => new Box();
";

const PY: &str = "@cache
def top(
    a: int,
    b: dict[str, int],
) -> int:
    \"\"\"Return the top. Ignore rest.\"\"\"
    def nested():
        pass
    return a


class Shape(Base):
    '''A shape.'''

    def area(self):
        return 0
";

fn item(
  name: &str,
  signature: &str,
  doc: Option<&str>,
  range: (usize, usize),
  (depth, public): (usize, bool),
) -> Item {
  Item {
    name: name.to_owned(),
    signature: signature.to_owned(),
    doc: doc.map(str::to_owned),
    start: range.0,
    end: range.1,
    depth,
    public,
  }
}

fn outlined(name: &str, text: &str) -> FileOutline {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join(name), text).unwrap();
  let mut files = outline(&Env::process(), dir.path(), &[name.to_owned()]).unwrap();
  assert_eq!(files.len(), 1);
  files.remove(0)
}

#[test]
fn outline_rust_keeps_items_and_methods_and_drops_locals() {
  let file = outlined("a.rs", RUST);
  assert_eq!(file.path, "a.rs");
  assert_eq!(file.lines, 26);
  assert_eq!(
    file.items,
    [
      item(
        "Point",
        "pub struct Point",
        Some("A point."),
        (1, 5),
        (0, true)
      ),
      item("Point", "impl Point", None, (7, 22), (0, false)),
      item("new", "pub fn new() -> Self", None, (8, 10), (1, true)),
      item(
        "scaled",
        "fn scaled<T>(&self, by: T) -> i32 where T: Into<i32>",
        None,
        (12, 21),
        (1, false)
      ),
      item("run", "fn run()", None, (24, 26), (0, false)),
    ]
  );
}

#[test]
fn outline_typescript_reads_jsdoc_and_decorators() {
  let file = outlined("b.ts", TS);
  assert_eq!(
    file.items,
    [
      item(
        "add",
        "export function add(a: number, b: number): number",
        Some("Adds two numbers."),
        (1, 11),
        (0, true)
      ),
      item("Box", "export class Box", None, (13, 19), (0, true)),
      item("size", "private size(): number", None, (15, 18), (1, false)),
      item(
        "make",
        "export const make = (n: number) => new Box()",
        None,
        (20, 20),
        (0, true)
      ),
    ]
  );
}

#[test]
fn outline_python_reads_docstrings_and_drops_nested_defs() {
  let file = outlined("c.py", PY);
  assert_eq!(
    file.items,
    [
      item(
        "top",
        "def top(a: int, b: dict[str, int]) -> int",
        Some("Return the top."),
        (1, 9),
        (0, true)
      ),
      item(
        "Shape",
        "class Shape(Base)",
        Some("A shape."),
        (12, 16),
        (0, true)
      ),
      item("area", "def area(self)", None, (15, 16), (1, true)),
    ]
  );
}

#[test]
fn outline_enclosing_returns_the_innermost_item() {
  let file = outlined("a.rs", RUST);
  assert_eq!(
    enclosing(&file, 20).map(|item| item.name.as_str()),
    Some("scaled")
  );
  assert_eq!(
    enclosing(&file, 4).map(|item| item.signature.as_str()),
    Some("pub struct Point")
  );
  assert_eq!(enclosing(&file, 6), None);
}

#[test]
fn outline_rejects_an_unsupported_language() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
  let error = outline(&Env::process(), dir.path(), &["notes.txt".to_owned()]).unwrap_err();
  assert_eq!(error, "unsupported language: notes.txt");
}

#[test]
fn outline_without_ast_grep_names_the_install_command() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("a.rs"), RUST).unwrap();
  let env = Env::process().with("PATH", &dir.path().join("empty").to_string_lossy());
  let error = outline(&env, dir.path(), &["a.rs".to_owned()]).unwrap_err();
  assert_eq!(error, MISSING);
  assert!(MISSING.contains("brew install ast-grep"));
}

#[test]
fn outline_batches_split_by_source_size_and_keep_order() {
  let sizes = [("a", 4), ("b", 4), ("c", 9), ("d", 1)];
  let groups = batches(
    sizes.iter().map(|(path, size)| ((*path).to_owned(), *size)),
    8,
  );
  assert_eq!(groups, [vec!["a", "b"], vec!["c"], vec!["d"]]);
}

#[test]
fn outline_skips_destructuring_bindings() {
  let file = outlined(
    "d.ts",
    "export const {\n  up,\n  down,\n} = slice.actions;\nexport const one = 1;\n",
  );
  let names: Vec<&str> = file.items.iter().map(|item| item.name.as_str()).collect();
  assert_eq!(names, ["one"]);
}

#[test]
fn outline_keeps_items_sharing_a_line_as_siblings() {
  let file = outlined("e.ts", "export const a = 1, b = 2;\n");
  let depths: Vec<(&str, usize)> = file
    .items
    .iter()
    .map(|item| (item.name.as_str(), item.depth))
    .collect();
  assert_eq!(depths, [("a", 0), ("b", 0)]);
}

#[test]
fn outline_leaves_out_a_file_whose_matches_pass_the_output_cap() {
  let dir = tempfile::tempdir().unwrap();
  // One 60 KB line with 200 bindings: each match repeats the line, about 12 MB of JSON.
  let pad = "x".repeat(280);
  let line: Vec<String> = (0..200)
    .map(|n| format!("export const v{n} = '{pad}';"))
    .collect();
  let line = line.concat();
  std::fs::write(dir.path().join("bundle.js"), format!("{line}\n")).unwrap();
  std::fs::write(dir.path().join("small.ts"), "export function ok() {}\n").unwrap();
  let paths = ["bundle.js".to_owned(), "small.ts".to_owned()];
  let files = outline(&Env::process(), dir.path(), &paths).unwrap();
  assert_eq!(files.len(), 1);
  assert_eq!(files[0].path, "small.ts");
  assert_eq!(files[0].items[0].name, "ok");
}
