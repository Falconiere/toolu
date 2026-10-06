use super::lex;
use crate::markdown_cli::words::Command;

/// Each command's name and line; the `case` tests share it.
pub(super) fn names(text: &str) -> Vec<(String, usize)> {
  lex(text, 1)
    .commands
    .into_iter()
    .map(|Command { line, words }| (words.first().cloned().unwrap_or_default(), line))
    .collect()
}

pub(super) fn owned(pairs: &[(&str, usize)]) -> Vec<(String, usize)> {
  pairs
    .iter()
    .map(|(name, line)| ((*name).to_owned(), *line))
    .collect()
}

/// plugins/jev/skills/jev/SKILL.md: assignments, a for header, an if chain.
const JEV_SETUP: &str = r#"# Codex
JEV="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/jev/jev.sh"
JEV_BUN=
for candidate in "${TOOLU_BUN:-}" "$(command -v bun 2>/dev/null)" "$HOME/.bun/bin/bun"; do
  if [ -n "$candidate" ] && [ -f "$candidate" ] && [ -x "$candidate" ]; then JEV_BUN="$candidate"; break; fi
done
"#;

#[test]
fn assignments_for_headers_and_keywords_run_nothing() {
  assert_eq!(
    names(JEV_SETUP),
    owned(&[("[", 5), ("[", 5), ("[", 5), ("break", 5)])
  );
}

/// plugins/ast-grep/skills/ast-grep/references/ast-grep-advanced.md: a
/// single-quoted YAML rule over five lines is one word.
const MULTILINE_QUOTE: &str = "mod.sh ast-grep scan 'id: find-async\nlanguage: rust\nrule:\n  kind: function_item\n  has:\n    pattern: async fn \\$NAME\n    stopBy: end'\nmod.sh ast-grep search 'x' --lang rust\n";

#[test]
fn a_quoted_word_spans_lines_and_keeps_its_quotes() {
  let lexed = lex(MULTILINE_QUOTE, 7);
  assert_eq!(lexed.commands.len(), 2);
  let first = &lexed.commands[0];
  assert_eq!((first.line, first.words.len()), (7, 4));
  assert!(first.words[3].starts_with("'id: find-async\nlanguage"));
  assert_eq!(lexed.commands[1].line, 14);
}

/// plugins/jev/skills/jev/references/problem-solving.md: a function, a
/// continuation, a redirection, a heredoc body that is never a command.
const FUNCTION_AND_HEREDOC: &str = r#"JEV_EXAMPLES=$(mktemp -d)

judge() {
  local name="$1"
  if "$JEV_BUN" "$JEV" ask "$JEV_EXAMPLES/$name.questions.json" \
      -s "@$JEV_EXAMPLES/$name.state.json" --raw >"$JEV_EXAMPLES/$name.result.json"; then
    jq '.answers' "$JEV_EXAMPLES/$name.result.json"
  fi
}
cat > "$JEV_EXAMPLES/x.json" <<'JSON'
{
  "type": "choice",
  "instructions": "Which?"
}
JSON
judge x
"#;

#[test]
fn functions_heredocs_continuations_and_redirections() {
  let lexed = lex(FUNCTION_AND_HEREDOC, 1);
  assert_eq!(lexed.functions, vec!["judge".to_owned()]);
  let found: Vec<(String, usize)> = lexed
    .commands
    .iter()
    .map(|command| (command.words[0].clone(), command.line))
    .collect();
  assert_eq!(
    found,
    owned(&[
      ("mktemp", 1),
      ("local", 4),
      ("\"$JEV_BUN\"", 5),
      ("jq", 7),
      ("cat", 10),
      ("judge", 16),
    ])
  );
  let ask = &lexed.commands[2].words;
  assert!(ask.contains(&"--raw".to_owned()), "{ask:?}");
  assert!(
    !ask.iter().any(|word| word.contains("result.json")),
    "the target drops: {ask:?}"
  );
  assert_eq!(lexed.commands[4].words, vec!["cat".to_owned()]);
}

#[test]
fn separators_split_commands_and_placeholders_stay_words() {
  let lexed = lex(
    "gh pr view <n> --json title | jq .title && git push; toolu epic answer <key> \"<text>\" 2>&1\n",
    3,
  );
  let words: Vec<Vec<String>> = lexed.commands.into_iter().map(|c| c.words).collect();
  assert_eq!(words[0], ["gh", "pr", "view", "<n>", "--json", "title"]);
  assert_eq!(words[1], ["jq", ".title"]);
  assert_eq!(words[2], ["git", "push"]);
  assert_eq!(words[3], ["toolu", "epic", "answer", "<key>", "\"<text>\""]);
}

#[test]
fn the_function_keyword_comments_braces_and_subshells() {
  let lexed = lex(
    "function setup {\n  (cd /tmp && ls) # list it\n}\nx=$(toolu --version)\n`date`\necho {a,b} ${HOME}\n",
    1,
  );
  assert_eq!(lexed.functions, vec!["setup".to_owned()]);
  let names: Vec<&str> = lexed.commands.iter().map(|c| c.words[0].as_str()).collect();
  assert_eq!(names, ["cd", "ls", "toolu", "date", "echo"]);
  assert_eq!(lexed.commands[4].words, ["echo", "{a,b}", "${HOME}"]);
}

#[test]
fn unterminated_quotes_and_heredocs_run_to_the_end() {
  assert_eq!(
    names("echo 'never closed\ngit push\n"),
    owned(&[("echo", 1)])
  );
  assert_eq!(names("cat <<EOF\ngit push\n"), owned(&[("cat", 1)]));
  assert_eq!(
    names("cat <<-EOF\n\tgit push\n\tEOF\ngh x\n"),
    owned(&[("cat", 1), ("gh", 4)])
  );
}

#[test]
fn escapes_trailing_backslashes_and_process_substitution() {
  assert_eq!(lex("echo a\\ b", 1).commands[0].words, ["echo", "a b"]);
  assert_eq!(lex("echo a \\", 1).commands[0].words, ["echo", "a"]);
  assert_eq!(
    names("diff <(sort a) b\n"),
    owned(&[("sort", 1), ("diff", 1)])
  );
  assert_eq!(
    lex("diff <(sort a) b\n", 1).commands[1].words,
    ["diff", "b"]
  );
  assert_eq!(
    names("x=$(toolu --version) && (cd a; ls) && git push\n"),
    owned(&[("toolu", 1), ("cd", 1), ("ls", 1), ("git", 1)])
  );
  assert_eq!(
    lex("echo \"a \\\" b\"", 1).commands[0].words,
    ["echo", "\"a \" b\""]
  );
  assert_eq!(names("echo ${unclosed\n"), owned(&[("echo", 1)]));
}

#[test]
fn array_assignments_are_not_commands() {
  assert_eq!(
    names(
      "files=(a.txt b.txt)
rm \"${files[@]}\"
"
    ),
    owned(&[("rm", 2)])
  );
  assert_eq!(
    names(
      "x+=(c)
"
    ),
    Vec::<(String, usize)>::new()
  );
}
