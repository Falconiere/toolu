# Shell command analysis

`@toolu/core/shell` gives every TypeScript gate one answer to the question "what does this Bash/Shell command run, and what does it write?" It parses the command once with **unbash 4.0.11** (pinned exactly, zero dependencies, inlined into each bundle) and returns flat records. No gate reads a command as text. Issue: [#284](https://github.com/Falconiere/toolu/issues/284). Defects it fixes: [#283](https://github.com/Falconiere/toolu/issues/283). Epic: [#247](https://github.com/Falconiere/toolu/issues/247).

## API

`writeTargets` lives in its own entry, `@toolu/core/shell/writes`. Only protected-files needs it, and a bundle that never asks what a command writes then does not carry it. Everything else below comes from `@toolu/core/shell`.

| Export | Answers |
|---|---|
| `analyzeShell(source)` | `{ commands, compoundRedirects, errors, unknown }`. It never throws. |
| `shellAnalysisOf(event)` | The same analysis for a `shell/pre` event, parsed on first use and shared by every module |
| `runsGitSubcommand(analysis, "push")` | `yes`, `no`, or `unknown` |
| `gitInvocation(command)` | The subcommand past git's global options, its arguments, and the `-C` chain |
| `pushTargets(analysis)` | Each push's cumulative `-C` chain, refspec, and destination branch |
| `commitMessages(invocation)` | Static `-m`/`--message` values, including `"$(cat <<'EOF' … EOF)"` |
| `writeTargets(analysis)` from **`@toolu/core/shell/writes`** | Paths written by redirects (every target, `/dev/null` included; a dynamic one is `path: null`), `tee`, `sed -i`, `perl -i`, `cp`/`mv`/`install` (the destination and each `DEST/basename(SRC)`, since `DEST` may be a directory), `dd of=`, and python (`-c STRING` or a static heredoc on stdin): a static `open(<literal>, <write mode>)` in any literal form reports its path, while any other `open(`, `Path(…).open`, `write_text`/`write_bytes`/`touch`, `shutil.copy*`/`move` or `os.rename`/`replace`/`symlink`/`link` reports an unknown target |
| `matchesRule(command, "node -e")` | An argv rule tested against one simple command |

Each `ShellCommand` carries the following fields:

- `words`: the name and arguments as written.
- `argv`: what actually runs once wrappers are removed.
- `patterns`: aligned with `argv`, the unexpanded text of each word bash globs.
- `texts`: aligned with `argv`, each word with quotes removed and expansions as written (`$HOME/.env`), so a dynamic path can still be matched by name.
- `wrappers`
- `redirects`
- `pipeline`: `{ index, size }`
- `exitProves`
- `origin`: `line`, `substitution`, `shell`, `eval`, or `function`
- `depth`
- `text`

A `null` word is dynamic: it expands at run time and is never guessed. That includes a pathname pattern, whose text is in `patterns`.

## What the walk covers

The walk covers every simple command in the line: inside `$(…)`, backticks, `<(…)`/`>(…)`, subshells, brace groups, `if`/`while`/`for`/`case` bodies, function bodies, `[[ ]]`, `(( ))`, `$(( ))`, parameter operands (`${X:-$(…)}`), assignment values, redirect targets, and unquoted heredoc bodies. A quoted heredoc body is data. The exception is a static heredoc or herestring fed to a shell (`bash <<'EOF'`), which is code.

The walk is a `switch` over every unbash node and word-part type, with a `never` default, so upgrading unbash to a version with a new node kind fails `tsc`.

Wrappers are unwrapped by their own option tables: `sudo`, `doas`, `env`, `command`, `builtin`, `exec`, `nohup`, `time`, `nice`, `timeout`, `xargs` and `stdbuf`. Two special cases:

- `command -v` runs nothing, so it is not unwrapped.
- `xargs` adds arguments at run time, so its argv ends with a `null`.

`bash|sh|zsh|dash|ksh -c STRING` (bash reads `+c` the same way, and `+o`/`-o` take a value) and `eval ARGS` are analyzed as command lines of their own, up to depth 4.

## Policy

- **Syntax errors:**
  - Commands unbash could read are reported, and errors come from the line and every nested script. `git push; echo "unterminated` is still a push.
  - A line with errors and no command at all (`)`, `if`) is `unknown`.
  - A line with errors exits non-zero in bash, so none of its commands has `exitProves`.
- **Dynamic names:**
  - `$g push`, `git $(echo push)` and `sudo $CMD` make `runsGitSubcommand` return `unknown`, never `no`.
  - A shell string that cannot be read statically (`bash -c "$CMD"`, `curl … | bash`, recursion past depth 4) becomes an unknown command (`argv: [null]`).
  - Each gate decides what unknown means for it. Security guardrails ask (block on Codex), and workflow gates keep today's behaviour.
- **Exit status:** `exitProves` holds only when a zero exit of the whole line proves that the command ran and exited zero:
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
- **Size:** input over 1 MiB is not parsed and is `unknown`. Measured: 1 MiB of dense script parses in about 15 ms, and a 1 MiB heredoc in 0.3 ms.
- **Out of scope:**
  - Shell state from earlier calls: aliases, functions, `cd`, `set -o pipefail`.
  - zsh-only syntax.
  - `env -S` strings (reported as unknown).
  - The contents of `source`d files and script files.
  - Writes by commands other than those `writeTargets` lists, such as `touch`, `truncate`, `ln`, `rsync`, `sponge` or an `awk` redirect inside its program. The bash implementation does not read these either.

## Consumers

- **gate-status** (PostToolUse, `@toolu/core/gates`, [#259](https://github.com/Falconiere/toolu/issues/259)):
  - It recognises quality commands from each command's `argv`, so prose that names one does not count.
  - It records a pass only when every recognised command has `exitProves`, and a failure only when at least one has it.
  - Wrappers, package runners and `bash -c` are followed.
- **push-waiver** (PostToolUse, #259): uses `isGitPush` and `pushTargetRoot` from `@toolu/core/detect` over the same analysis.

## Fixtures and the bash oracle

`tooling/fixtures/shell/` holds real inputs (see its README):

- `bats-parity.json`: 186 inputs the six shipped bash functions received from 15 bats suites (`is_git_push`, `is_git_commit`, `bash_write_targets`, `bash_commands_decide`, `push_target_root`, `push_target_branch`). The TypeScript layer returns the same answer for all of them. Push roots and branches are checked against real git repositories. There are two documented differences. `bash_write_targets` over-includes the sed/perl script operand, and `cp`/`mv`/`install` also report `DEST/basename(SRC)`, because `DEST` may be an existing directory.
- `issue-283.json`: 49 named fixtures, at least one per #283 item. Each records the correct result and the bash result as the known-wrong baseline.

`packages/toolu-core/src/shell/__tests__/bash-oracle.test.ts` sources the unmodified `detect.sh` and `bash-commands.sh` and re-derives every live baseline on each run. It is deleted with the bash implementation (#279).

## Budget evidence

`bun run bench:shell` builds probe hook entries with the plugin bundle pipeline (`stageBundles`):
- `empty`, the baseline;
- `shell`, which imports and calls every runtime export of `@toolu/core/shell`;
- `writes`, which is `analyzeShell` plus `@toolu/core/shell/writes`, the entry protected-files needs;
- `together`, every public export of both entries in one bundle, as the PreToolUse dispatcher (#258) carries them.

It runs `empty` and `together`, the heaviest, interleaved from a directory with no `node_modules`, and times `analyzeShell` plus the git and write helpers over every fixture command. `--assert` exits 1 when a budget is exceeded.

CI asserts the bundle sizes (`tooling/src/__tests__/bench-shell.test.ts`), because wall-clock numbers depend on the machine.

| Measure | Budget | Measured |
|---|---|---|
| Bundle size added, every runtime export of `@toolu/core/shell` (unminified) | ≤ 200,000 B | 197,434 B |
| Bundle size added, `analyzeShell` + `@toolu/core/shell/writes` (unminified) | ≤ 200,000 B | 199,180 B |
| Bundle size added, both entries together (unminified) | ≤ 205,000 B, by product-owner decision | 203,159 B |
| Cold-start p50, `together` minus `empty` (40 interleaved runs) | ≤ 5 ms | +4.16 ms (empty 20.61 ms, together 24.78 ms; p90 22.96 / 26.55 ms) |
| Parse and walk over 235 fixture commands, 4,700 samples | p99 ≤ 0.1 ms | p50 3.3 µs, p99 17.4 µs, max 2.1 ms |

Measured on 2026-09-29 with Bun 1.4.2 on macOS 26.6.2 (darwin arm64, Apple M2 Max), with other agent sessions on the same machine (load average about 2.6). Load raises both absolute cold-start numbers alike, and the budget is the difference between them. For comparison, the shipped `is_git_push` takes 0.23 s under bash 5.3 and 0.61 s under `/bin/bash` 3.2 on the 4.3 KB fixture `283-11a`.

**Where the bytes go.**
- The combined bundle's 203,159 B, per module:
  - unbash 174,613 B: lexer 107,086, parser 46,219, arithmetic 13,152, ansi-c 3,243, word 2,553, parts 1,550, chars 810. It is pinned, and its lexer is one class a bundler cannot trim.
  - the analyzer 27,789 B: words 5,685, writes 5,676, walk 5,613, argv 3,770, git 2,483, options 2,285, parse 1,169, rules 625, event 309, types 174. No module appears twice, and Bun strips comments.
  - the probe's own code, about 0.76 KB.
- The analyzer grew past the issue prototype's 10 KB because review found cases the prototype missed:
  - bash globs unquoted redirect targets and arguments (`> .en[v]` writes `.env`);
  - dynamic targets keep a matchable text;
  - `cp`/`mv`/`install` into a directory;
  - `env -` and `env -P`;
  - `xargs` running a command zero times.
- Three structural changes brought it down without dropping any of those fixes:
  - one-pass word scanning, which resolves a value and visits nested scripts in a single exhaustive switch;
  - a single walk switch and a single-loop option parser;
  - table-driven writers.
- `writeTargets`, the largest single-consumer piece, moved to its own entry.
- The shell option reader now reuses `parseArgs` rather than duplicating it (−239 B), which also makes `bash +c` analyzed like `-c`.
- Both entries together stay above 200 KB: unbash leaves the analyzer about 25 KB, and structural work cannot recover the rest without shortening names or dropping coverage. The product owner set that bundle's budget at 205,000 B; each entry alone stays within 200,000 B.
