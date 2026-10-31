# Shell command analysis

`toolu_shell::analyze` (`crates/core/shell`) gives every native gate one answer to the question "what does this Bash/Shell command run, and what does it write?" It parses the command once with tree-sitter-bash (see **The Rust implementation**) and returns flat records. No gate reads a command as text. Issues: [#284](https://github.com/Falconiere/toolu/issues/284) (the analyzer, first written in TypeScript on unbash 4.0.11 and since removed), [#416](https://github.com/Falconiere/toolu/issues/416) (the Rust port). Defects it fixes: [#283](https://github.com/Falconiere/toolu/issues/283). Epic: [#247](https://github.com/Falconiere/toolu/issues/247).

## API

`write_targets` lives in its own module, `toolu_shell::writes`. Everything else below comes from the crate root and `toolu_shell::git` and `toolu_shell::rules`. The engine parses a `shell/pre` event once per path walk and reuses that analysis for every gate in the walk (#759). The next hook event, the next path of a multi-file patch, and a changed host, working directory, or repository parse and load config again. Each gate still receives the loader's warnings, and an invalid envelope still fails closed. bash-commands and protected-files still skip parsing when their lists are empty.

| Export | Answers |
|---|---|
| `analyze(source)` | `ShellAnalysis { commands, compound_redirects, errors, unknown }`. It never panics. |
| `runs_git_subcommand(analysis, "push")` | `yes`, `no`, or `unknown` (`Tristate`) |
| `git_invocation(command)` | The subcommand past git's global options, its arguments, and the `-C` chain |
| `push_targets(analysis)` | Each push's cumulative `-C` chain, refspec, and destination branch |
| `commit_messages(invocation)` | Static `-m`/`--message` values, including `"$(cat <<'EOF' … EOF)"` |
| `commit_args(invocation)` | Every message source of a commit: `-m` values, `-F` files with the static heredoc or herestring `-F -` reads, `--trailer` values, whether it amends, and its `MessageOrigin`: written here, reused by `-C`/`-c`, derived by `--fixup`/`--squash`, or kept by `--amend --no-edit` |
| `new_branches(invocation)` | Branch names introduced by `checkout -b/-B/--orphan`, `switch -c/-C/--create/--force-create/--orphan`, `branch <new>`, `branch -m/-M/-c/-C [<old>] <new>` and `worktree add -b/-B`; listing, deleting and checking out an existing branch name none, and a dynamic name is `None` |
| `write_targets(analysis)` from **`toolu_shell::writes`** | Paths written by redirects (every target, `/dev/null` included; a dynamic one has no path), `tee`, `sed -i`, `perl -i`, `cp`/`mv`/`install` (the destination and each `DEST/basename(SRC)`, since `DEST` may be a directory), `dd of=`, and python (`-c STRING` or a static heredoc on stdin): a static `open(<literal>, <write mode>)` in any literal form reports its path, while any other `open(`, `Path(…).open`, `write_text`/`write_bytes`/`touch`, `shutil.copy*`/`move` or `os.rename`/`replace`/`symlink`/`link` reports an unknown target |
| `matches_rule(command, "node -e")` | An argv rule tested against one simple command |

Each `ShellCommand` carries the following fields:

- `words`: the name and arguments as written.
- `argv`: what actually runs once wrappers are removed.
- `patterns`: aligned with `argv`, the unexpanded text of each word bash globs.
- `texts`: aligned with `argv`, each word with quotes removed and expansions as written (`$HOME/.env`), so a dynamic path can still be matched by name.
- `wrappers`
- `redirects`
- `pipeline`: `{ index, size }`
- `exit_proves`
- `origin`: `line`, `substitution`, `shell`, `eval`, or `function`
- `depth`
- `text`

A `None` word is dynamic: it expands at run time and is never guessed. That includes a pathname pattern, whose text is in `patterns`.

## What the walk covers

The walk covers every simple command in the line: inside `$(…)`, backticks, `<(…)`/`>(…)`, subshells, brace groups, `if`/`while`/`for`/`case` bodies, function bodies, `[[ ]]`, `(( ))`, `$(( ))`, parameter operands (`${X:-$(…)}`), assignment values, redirect targets, and unquoted heredoc bodies. A quoted heredoc body is data. The exception is a static heredoc or herestring fed to a shell (`bash <<'EOF'`), which is code.

The walk maps tree-sitter's nodes to bash's semantics; where tree-sitter reads a script differently from bash, the crate corrects it (see **The Rust implementation**).

Wrappers are unwrapped by their own option tables: `sudo`, `doas`, `env`, `command`, `builtin`, `exec`, `nohup`, `time`, `nice`, `timeout`, `xargs` and `stdbuf`. Two special cases:

- `command -v` runs nothing, so it is not unwrapped.
- `xargs` adds arguments at run time, so its argv ends with a `None`.

`bash|sh|zsh|dash|ksh -c STRING` (bash reads `+c` the same way, and `+o`/`-o` take a value) and `eval ARGS` are analyzed as command lines of their own, up to depth 4.

## Policy

- **Syntax errors:**
  - Commands the parser could read are reported, and errors come from the line and every nested script. `git push; echo "unterminated` is still a push.
  - A line with errors and no command at all (`)`, `if`) is `unknown`.
  - A line with errors exits non-zero in bash, so none of its commands has `exit_proves`.
- **Dynamic names:**
  - `$g push`, `git $(echo push)` and `sudo $CMD` make `runs_git_subcommand` return `unknown`, never `no`.
  - A shell string that cannot be read statically (`bash -c "$CMD"`, `curl … | bash`, recursion past depth 4) becomes an unknown command (`argv: [None]`).
  - Each gate decides what unknown means for it. Security guardrails ask (block on Codex), and workflow gates keep today's behaviour.
  - protected-files (#260) checks every write target it can name:
    - a static path;
    - every existing file a pathname pattern matches, plus the pattern itself;
    - a dynamic target by its text (`> $HOME/.env` matches `.env`).

    A write it cannot name, such as `bash -c "$CMD"` or a python write whose path is computed, is not prompted. The bash gate did not prompt for it either, and asking on every `> "$OUT"` would be a behaviour change, which is out of scope for #247.
- **Exit status:** `exit_proves` holds only when a zero exit of the whole line proves that the command ran and exited zero:
  - the last statement, not backgrounded;
  - an and-or element with `&&` on both sides;
  - the last element of a pipeline that is not negated;
  - through subshells, groups, `bash -c` and `eval`.
  
  A command run by `xargs` never proves anything, because `xargs -r` can run it zero times and still exit 0.
  
  `bun test | tail` does not prove `bun test` passed.
- **Pathname patterns:** bash globs an unquoted word that contains `*`, `?` or `[…]` and uses the one existing file it matches. It does this for a redirect target or argument too: `echo x > .en[v]` writes `.env`, which was verified with neutral names under bash 5.3 and `/bin/bash` 3.2. Such a word is therefore not a static value:
  - its entry in `words`/`argv` is `null`, so `/usr/bin/g[i]t push` is `unknown`, and its pattern is kept in `patterns`;
  - on a redirect it is in `ShellRedirect.pattern`, and on a write target in `WriteTarget.pattern`.
  
  A guardrail must treat a pattern as every path it matches. Quoted or backslash-escaped characters are literal.
- **Size:** input over 1 MiB is not parsed and is `unknown`.
- **Out of scope:**
  - Shell state from earlier calls: aliases, functions, `cd`, `set -o pipefail`.
  - zsh-only syntax.
  - `env -S` strings (reported as unknown).
  - The contents of `source`d files and script files.
  - Writes by commands other than those `write_targets` lists, such as `touch`, `truncate`, `ln`, `rsync`, `sponge` or an `awk` redirect inside its program. The bash implementation does not read these either.

## Consumers

- **gate-status** (PostToolUse, `crates/core/engine/src/gates/gate_status.rs`, [#259](https://github.com/Falconiere/toolu/issues/259)):
  - It recognises quality commands from each command's `argv`, so prose that names one does not count.
  - It records a pass only when every recognised command has `exit_proves`, and a failure only when at least one has it.
  - Wrappers, package runners and `bash -c` are followed.
- **push-waiver** (PostToolUse, #259): uses `is_git_push` and `push_target_root` from `toolu_engine::detect` over the same analysis.

## Fixtures

`fixtures/shell/` holds real inputs (see its README):

- `bats-parity.json`: 186 inputs the six shipped bash functions received from 15 bats suites (`is_git_push`, `is_git_commit`, `bash_write_targets`, `bash_commands_decide`, `push_target_root`, `push_target_branch`). `toolu_shell` returns the same answer for all of them. Push roots and branches are checked against real git repositories. There are two documented differences. `bash_write_targets` over-includes the sed/perl script operand, and `cp`/`mv`/`install` also report `DEST/basename(SRC)`, because `DEST` may be an existing directory.
- `issue-283.json`: 49 named fixtures, at least one per #283 item. Each records the correct result and the bash result as the known-wrong baseline.

The bash oracle test, which sourced the unmodified `detect.sh` and re-derived every live baseline, was deleted with the bash implementation (#279). The TypeScript replay went with the TypeScript implementation (#440). `crates/core/shell/tests/fixture_cases.rs` replays both files and `analysis_fixture.rs` replays `analysis.json`.

## The Rust implementation: `toolu-shell`

`crates/core/shell` ([#416](https://github.com/Falconiere/toolu/issues/416)) ported the TypeScript analyzer for the Rust hooks of epic #402, and #440 removed the original. The per-walk cache lives in the engine (`crates/core/engine/src/gates/event_memo.rs`, #759). `push_target_root` and `push_target_branch` spawn git and stay with the detect port; `crates/core/shell/tests/fixture_cases.rs` replays them over real repositories from the crate's answers.

**Parser.** The crate uses tree-sitter 0.24.7 with tree-sitter-bash 0.23.3. `brush-parser`, the issue's first choice, fails `deny.toml` in every release: it brings syn 2 beside the workspace's syn 3, duplicate darling and hashbrown versions, and the Zlib-licensed `foldhash`. Every tree-sitter from 0.25 to 0.27 build-depends on `serde_json` with `preserve_order`. In this workspace that reaches the same `foldhash` through `indexmap` and `hashbrown`. 0.24.7 is the last release without it, and tree-sitter-bash 0.23.3 is the last grammar it can load (ABI 14).

**Vendored grammar.** `vendor/tree-sitter-bash` is 0.23.3 with one fix backported from 0.25.1. The scanner serializes its heredoc stack (4 bytes, then 7 and the delimiter with its NUL per heredoc) into tree-sitter's 1024-byte buffer, and its bounds check leaves out the 4-byte length. A state of 1,025 to 1,027 bytes then fails tree-sitter's assertion, which aborts the process, or overruns the buffer. Fuzzing found it twice. The stack also keeps stale entries when the parser restores an earlier version, and error recovery reads delimiters again at other offsets, so no bound computed from the text holds: three such bounds failed, the last after measuring 73,010 inputs. With the fix, `serialize` stops before the buffer is full, and every input that aborted 0.23.3 parses (`tests/limits.rs`). The vendored crate is excluded from the workspace and builds only as `toolu-shell`'s dependency. `cargo xtask check-reach` leaves out a crate the root manifest excludes under `vendor/`, and release-please, `validate-plugin-packaging` and `release_native.py` leave its lockfile version at 0.23.3. Drop it once a tree-sitter-bash with the fix fits `deny.toml`. The walk maps tree-sitter's nodes to unbash's semantics. Where tree-sitter reads a script differently from bash, the crate corrects it:

- **Before parsing** (`fixup.rs`), the script is parsed again with the difference removed, at the same length so every offset holds:
  - one `time [-p]` before a pipeline is blanked, as bash reads it as a keyword;
  - a `[`, `[[` or `{` glued to the next character (`[g]it push`, `{node,} -e x`) is a pattern or brace word, not a test or group;
  - words and redirects after a heredoc delimiter (`cat <<EOF a > .env`) swap places with `<<EOF`.
- **While walking:**
  - a word split at an escaped separator (`node -\` and a newline then `e`, `'node'\ '-e x'`) is one word;
  - only the first word after `>` is the target (`echo x > .env y`);
  - `<>`, a descriptor glued to its redirect (`0<file`, `{fd}>file`) and a `-` dropped before `<<` are restored;
  - a redirect tree-sitter puts on a whole list (`a && b 2>&1 | c`) goes to its last command;
  - a backtick body that escapes a backtick is decoded and parsed again;
  - `(( … ))` read as a test command (`((x++)) 2>&1`) runs nothing, and the redirects of `(( … ))` and `[[ … ]]` are compound redirects (`(( 1 )) > .env` writes `.env`);
  - a here-string tree-sitter leaves unlabelled on a compound statement (`done <<< "$(git push)"`) is still its redirect;
  - `coproc` before a compound body (`coproc (git push)`) runs the body in the background;
  - a digit glued to `<<` is the heredoc's descriptor (`0<<EOF git push`);
  - a heredoc line keeps the words after a redirect (`a=1 <<-EOF >f rm -rf x`) and the whole pipeline after `&&` (`cat <<EOF && a | git push`);
  - a bare `$` tree-sitter split from the name after it (`\"$b`) still expands;
  - braces expand across quotes (`{"git","push"}` is two dynamic words).

**Fail closed.** Any ERROR or MISSING node left after the fixups makes the analysis `unknown`; the commands read are still reported. The removed TypeScript analyzer was unknown only when it read no command, but tree-sitter-bash reports ERROR nodes for valid bash too, and the commands around one can be merged or missing: `cat <<'EOF'; git push` put `git push` into `cat`'s arguments, `3<<EOF git push` named the command `EOF`, `echo "a``" > .env` lost the write. Some misreads carry no ERROR node and are errors too:
- two backtick substitutions tree-sitter reads as one (`` echo `ls` `git push` ``);
- a `[` whose lines run on to `]` (bash ends `[` at the newline and runs the next line);
- a heredoc body line starting with `\` that tree-sitter reads into the delimiter's line, losing it from the body (`bash <<EOF`, then `\git push`).

**Limits.** All of these make the analysis `unknown`, which a guardrail treats as "ask":
- `MAX_NESTING` (64): nested scripts and compound bodies past this depth. unbash, the removed TypeScript parser, stopped at 256 `$(…)` levels without marking the line unknown.
- `PARSE_BUDGET` (1 s, wall clock, shared by every nested parse and the walk): tree-sitter does not check its timeout everywhere, and some inputs take time quadratic in their length (20,000 commands then a trailing `|` took 11 s, one 80 KB heredoc line 20 s). A script of 4 KiB or more is parsed on a worker thread the analysis stops waiting for at the deadline; the walk stops there too.

**Parity.** `fixtures/shell/analysis.json` holds the removed TypeScript analyzer's projected analysis of every input of `unbash-baseline.json`. `crates/core/shell/tests/analysis_fixture.rs` reproduces it. The fixture records two intended differences from that analyzer:

- `echo $(unterminated`: Any parse error makes the Rust analysis unknown, where the removed TypeScript analyzer trusted the commands it read: tree-sitter-bash reports ERROR nodes for valid bash too (`cat <<EOF; git push`), so the commands read around one may be merged or missing. They are still reported, and bash runs nothing from a line it cannot parse.
- `echo 'unterminated`: tree-sitter-bash leaves an unterminated quote in an ERROR node instead of folding it into the command's last word, so the command is reported without the broken word. Like every parse error, it makes the Rust analysis unknown.

Other known differences, found by a differential run over 1,526 inputs (the fixtures, every string literal in the TypeScript shell, detect and gate tests, and adversarial cases). Each is malformed input or a form tree-sitter-bash 0.23 cannot read, and each leaves Rust no less cautious:

- Error recovery: the words of a command broken by a syntax error, and the order or origin of commands inside a broken substitution. Rust marks such a line unknown.
- Extended globs (`@(…)`, `!(…)`), and a pathname or brace pattern as a command name that tree-sitter cannot read (`a?c`, `[[:alpha:]]*`): Rust reports an error or `unknown` where unbash reads a pattern.
- Two heredocs on one command, or an unterminated heredoc.
- A carriage return: tree-sitter treats it as whitespace, while bash and unbash keep it in the word.
- After a swapped heredoc, the command's `text` and redirect order follow the rewritten line.
- A python `open()` whose mode does not close at its first quote (`'w' if a else 'r'`) is a write to an unknown path; the removed TypeScript analyzer tried every later quote, which was quadratic.
- More than 4 MiB of copy targets (many sources into a long destination) are the destination and one unknown target.
- Error messages and offsets are tree-sitter's (offsets are bytes, not UTF-16 units). The oversize message is the removed TypeScript analyzer's.

**Latency.** `cargo test --release -p toolu-shell --test latency` runs `analyze` plus the git and write helpers over the 235 real commands of the shell fixtures. Measured on 2026-10-06 on a shared Linux x86_64 host: p50 12.5 µs, p99 48.6 µs, against a 100 µs budget.

**Fuzzing.** `crates/core/shell/fuzz` is a cargo-fuzz (libFuzzer) package on nightly. It has two targets:
- `analyze`: arbitrary input, then every helper a gate calls.
- `nested`: the input bytes open, fill and close quotes, substitutions, groups, heredocs and shell strings.

To run a target from that directory:

```bash
CC=clang CFLAGS="-fsanitize=fuzzer-no-link,address" cargo fuzz run analyze -- -max_total_time=60
```

clang instruments tree-sitter's C sources for coverage and AddressSanitizer. The `fuzz` job in `tests.yml` runs each target for 60 seconds on every Rust change. `.github/workflows/fuzz.yml` runs each for 30 minutes, daily and on demand. Both seed the corpus from the shell fixtures.
