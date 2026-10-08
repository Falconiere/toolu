# Jev in Rust — Design

**Date:** 2026-10-08   **Status:** Approved   **Author:** epic worker   **Topic:** `toolu jev` and native jev hooks

## Problem

`plugins/jev` still runs as Bun bundles. Every session pays to start Bun, and the agent is told to call a bundle. The core client in `toolu-jev-client` already performs the HTTP call. This issue puts the CLI and the hooks on that client and deletes the bundles.

## Non-Goals

1. Curl-compatible process statuses 22 and 28. `toolu` exits only through `Exit` (0, 1, 2, 64, 69, 75).
2. Removing Bun from any plugin other than jev, or changing the launcher's bundle fallback (#440).
3. A new endpoint environment variable. Tests pass `Config` into the library entry the binary calls.
4. Rewriting the OpenCode bootstrap. Tests and generated skill copies that name the deleted bundle are updated to the shim and to `toolu jev`.

## Architecture

`crates/jev` (`toolu-jev`) replaces `Planned` with real clap verbs and three hooks. Split the crate so no file passes 300 code lines and no function passes 50: `cli.rs` (the clap tree), `call.rs` (matches to `State` and `Questions`), `present.rs` (reply and error to `Outcome`), `session.rs`, `prompt.rs`, and `check.rs`, each with a wired `tests/<module>_test.rs`. `lib.rs` keeps `PLUGIN`, `command`, `run`, `needs_stdin`, and `execute`. `run` builds `Env::process()` and `Config::default()` and calls `execute`, which the tests call with a loopback `Config`. When `Config.test_root_ca_der` is unset and `NODE_EXTRA_CA_CERTS` names a PEM file, `toolu-http` parses that certificate to DER and the client trusts only that root. That is the environment the existing HTTPS fixture already sets, so the OpenCode live test can run the shim. Unset, the webpki roots stay. This is not an endpoint override. `execute` uses `toolu_jev_client::Jev`. `crates/cli` already registers `toolu_jev::command` and `toolu_jev::run`. Hook dispatch in `crates/cli/src/hook.rs` calls `toolu_jev` for `session-start`, `user-prompt-submit`, and `check-binary` instead of the "has no hook" message.

`plugins/jev/scripts/jev.sh` is `#!/bin/sh` plus `exec toolu jev "$@"`, mode `0755`. SessionStart links it with `toolu_runtime::startup::publish`. A regular file already at that path is left alone.

The native-binary notice moves to `toolu_runtime::startup` as `native_toolu_advice`, a port of `packages/toolu-core/src/startup/native-toolu.ts`. The notice filename is a stable hex FNV-1a of the session id, so no new crate is added. Jev's `check-binary` hook only renders that line.

Stdin stays in `crates/cli`. `Ctx` gains `stdin: Option<String>` (default `None`). When `toolu_jev::needs_stdin` is true, dispatch reads stdin with `toolu_protocol::stdin::read_stdin` and sets the field. The plugin crate does not touch stdout, stderr, or `std::env`.

`plugins/jev/hooks/src` and `plugins/jev/hooks/dist` are deleted only after the Rust tests that port them are written and passing. Until that deletion, those TypeScript files stay the oracle for the cases named below. `hooks.json` is three entries from `cargo xtask print-hook`. The generated command still mentions a Bun fallback path; that path is unused once `hooks/dist` is gone, and it is not a requirement to keep Bun.

## Interfaces / Schema

Verbs, sharing `-s/--state` (required), `-m/--model` (default `jev-latest`), and `--raw`:

| Verb | Positionals and flags |
| --- | --- |
| `noul` | `<instructions>`, `--true`, `--false`, `--id` (default `q`) |
| `choice` | `<instructions>`, repeatable `-o/--option` `KEY` or `KEY=DESC` (2..=255), `--id` |
| `score` | `<instructions>`, repeatable `-l/--level` (2..=10, lowest first), `--id` |
| `ask` | `<questions.json>` or `-`, no `--id` |

`--state -` and `ask -` read `Ctx.stdin`. `--state @FILE` and a questions path read that file. Trailing line feeds are stripped. `@FILE` and stdin state use `State::structured`. A questions file uses `Questions::parse`.

Success stdout is one compact JSON object of the reply's `answers`, question order preserved, plus the newline `crates/cli` adds. `--raw` prints the reply body pretty-printed (two-space indent). `--json` does not wrap a success that already has stdout. On failure, `--json` still adds the CLI error envelope on stdout; stderr is unchanged. Without `--json`, failure stdout is empty. `--quiet` drops stderr on success.

`hooks.json` keeps the matchers `startup|resume|clear|compact` on both SessionStart entries and no matcher on UserPromptSubmit. Each hook object is the `cargo xtask print-hook` entry, including its Bun fallback path. That path is why the gate-coverage id stays `hooks/dist/<name>.js`; the file itself is not restored. `hostMechanism` becomes `native`.

The SessionStart and UserPromptSubmit bodies stay the current paragraphs, with two substitutions: the called command is `toolu jev` when the published path is our symlink, or the shell-quoted user path when the path is a kept user file; the sentence about Bun is replaced by `The command is toolu jev and does not load a project .env file.` OpenCode still cites `skill({ name: "jev-jev" })`; other hosts cite `<plugin>/skills/jev/SKILL.md`. `credentialNotice` is unchanged. OpenCode compaction (`source` `compact`) only relinks and prints no mandate.

`check-binary` prints nothing when `command -v toolu` is a native binary. Otherwise it prints one `SessionStart` context line: the absolute path, or the install line with `curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash` and `brew install falconiere/tap/toolu`. The session id is the payload string `session_id`, otherwise `TOOLU_SESSION_ID`. A missing id prints the line every time. A present id prints it once, recorded under the config root.

Hook stdout is `startup::render_hook_output` of `session_context`, not a `systemMessage`. A SessionStart skew advisory is a `systemMessage` line before that JSON. Unreadable or non-object hook stdin is a normal start: SessionStart still prints the mandate, and it is not treated as compaction. Two publishes of the same symlink use the existing atomic replace; a user file is still kept.

## Failure modes and edge cases

| Case | Stdout | Stderr | Exit |
| --- | --- | --- | --- |
| Missing or empty key | empty | `jev: TYPESAFE_API_KEY unset` | 1 |
| Key contains CR or LF | empty | `jev: TYPESAFE_API_KEY must not contain line breaks` | 1 |
| Client `InvalidQuestion` or `InvalidResponse` | empty | `jev: ` plus the client `Display` | 1 |
| Unreadable `@FILE`, questions path, or stdin | empty | `jev: cannot read <path>` or `jev: cannot read stdin` | 1 |
| Clap usage (unknown flag, missing verb, missing `--state`) | empty unless `--json` | clap's message | 64 |
| Terminal HTTP status | empty without `--json` | raw body, key redacted by the client | 1 |
| Timeout after the attempts | empty without `--json` | `jev: the Jev request timed out` | 75 |
| Retryable status, then a typed answer | the answer map | empty | 0 |
| Other transport failure | empty | `jev: ` plus the client `Display` | 1 |

Stdout cells are without `--json`. With `--json`, a failure also prints the CLI error envelope. No failure prints a judgment. The key never appears in stdout, stderr, or a panic. A non-executable published path yields the existing "Jev unavailable" mandate instead of `toolu jev`. An unwritable config directory prints `jev: cannot create <dir> — wrapper not published` on stderr, exits 0, and prints no context. A failed relink prints `jev: cannot publish <path>` on stderr, exits 0, and prints no context. A missing shim source prints nothing and publishes nothing. Trivial prompts match the current regex and print nothing. A prompt hook whose wrapper is missing or not executable prints nothing.

## Acceptance criteria

- **AC-1:** Given `TYPESAFE_API_KEY` and a loopback reply `{"model":"jev-1.13.0","answers":{"q":{"type":"noul","noul":0.92}},"usage":{"input_tokens":3,"output_tokens":2}}`, `toolu jev noul -s "Payouts failing for 3 days" "Urgent?"` prints `{"q":{"type":"noul","noul":0.92}}` and exits 0. The recorded request body is `{"state":"Payouts failing for 3 days","model":"jev-latest","questions":{"q":{"type":"noul","instructions":"Urgent?"}}}`.
- **AC-2:** Given no key, `toolu jev choice "q" -s STATE -o a -o b` exits 1, stderr is `jev: TYPESAFE_API_KEY unset`, stdout is empty, and no request is sent. A key containing a line break exits 1 with `jev: TYPESAFE_API_KEY must not contain line breaks`.
- **AC-3:** A 401 body `{"error":"bad key"}` exits 1 with that body on stderr and empty stdout. `JEV_TIMEOUT=0` exits 75 with empty stdout and no judgment.
- **AC-4:** SessionStart on Claude, Codex (`CODEX_HOME` with a space), and OpenCode publishes a symlink whose target text is `#!/bin/sh` and `exec toolu jev "$@"`. The context tells the agent to run `toolu jev`, contains no `bun` and no API key, and OpenCode cites `skill({ name: "jev-jev" })`. `TOOLU_CONFIG_DIR` wins over the Codex root. OpenCode `source: compact` relinks and prints no mandate; Claude compaction still prints it. A regular file at `jev.sh` is not replaced, and the context quotes that path. A missing shim source publishes nothing and prints nothing. An unwritable directory or a refused link prints one stderr line and no context.
- **AC-5:** UserPromptSubmit with prompt `rank these approaches` includes `toolu jev` and the skill reference. Prompt `LGTM` and the other trivial forms print nothing.
- **AC-6:** `check-binary` prints nothing when the shell's `toolu` is native. Otherwise it prints one install or absolute-path line, and a second call with the same `session_id` prints nothing.
- **AC-7:** `plugins/jev/hooks/src` and `plugins/jev/hooks/dist` are gone. `hooks.json` matches `cargo xtask print-hook`. The plugin has a skill and a README and no `commands/` or `agents/` directory. Those Markdown files and `tools/toolu-opencode/generated/skills/jev-jev` say `toolu jev` and do not name `hooks/dist`, a Bun bundle, or `bun`. `docs/cli/` documents the four verbs.
- **AC-8:** Given a loopback choice reply whose `answers.q` is `{"type":"choice","choice":"billing","probabilities":{"billing":0.8,"tech":0.2},"confidence":0.8}`, `toolu jev choice -s ticket "Which team?" -o billing=Payments -o tech` prints that answers object and exits 0. The request `criteria` is `{"billing":"Payments","tech":null}`.

## Acceptance evidence

- **AC-1:** `crates/jev/tests/cli.rs` against `toolu_http_test_support::Fixture` (real TLS, CONNECT proxy, fixture CA on `Config`). Port every case in `plugins/jev/hooks/src/__tests__/jev.test.ts`: criteria shapes, `__proto__` as a data key, `ask` from stdin with `-m`, structured `@FILE` state, upper bounds before any request, a malformed body, a missing answer, `Retry-After: 61` returning the body on the first attempt, and a dropped connection then a typed answer. Status 22 in that file is exit 1 here, and status 28 is exit 75.
- **AC-2:** The same test, plus `crates/cli` spawning the built `toolu` binary with the key unset.
- **AC-3:** The same fixture: one 401 route; `JEV_TIMEOUT=0` with a 10 ms pause so the three attempts do not wait a minute. A 408 followed by the AC-1 body exits 0 and records two requests.
- **AC-8:** The same fixture and the choice reply above. Stdout is the compact answers object.
- **AC-4 / AC-5:** `crates/jev` hook tests with temp homes and `Env::from_pairs`. Port every case in `session-start.test.ts`, `user-prompt-submit.test.ts`, and `opencode.test.ts`, with the called command changed from the Bun wrapper invocation to `toolu jev` (or the quoted user path). That includes a slash command, a multiline prompt, invalid prompt input, a stale link refreshed in place, and a non-executable user file.
- **AC-6:** A runtime unit test of `native_toolu_advice` with a temp `PATH` and a real executable whose `--hook-protocol` prints `1`, an empty `PATH`, and two calls with the same session id.
- **AC-7:** `cargo xtask check-hooks`, `cargo xtask docs-cli` drift, `cargo xtask check-markdown-cli`, and `bun run check:plugin-bundles`.

`bun run test:unit` bootstraps the real jev plugin, and the OpenCode live test runs the published shim. Both need `toolu` on `PATH`. The `ts` job already installs the toolchain; add `cargo build -p toolu-cli` and put `target/debug` on `PATH` before `bun run test:ts`. The `opencode` job gains the toolchain, the same build, and the same `PATH`, because it has no Rust setup today. Update bootstrap expectations from `jev/hooks/dist/jev.js` to `jev/scripts/jev.sh`. In `fixtures/gate-coverage/inventory.json` the three jev ids stay, because the generated command still names `hooks/dist/<name>.js`; set `hostMechanism` to `native` and run `bun run tooling/src/gate-coverage-inventory.ts render` so `docs/gate-coverage-matrix.md` matches.

## Documentation impact

- `plugins/jev/skills/jev/SKILL.md` and `references/problem-solving.md`: CLI section is `toolu jev`, no Bun resolver.
- `plugins/jev/README.md`: prerequisite is the `toolu` install, not Bun; the wrapper is the shim.
- `tools/toolu-opencode/generated/skills/jev-jev/`: the same text, via the surface generator if that is how the copy is produced.
- `docs/cli/`: regenerated from the binary.
- `docs/gate-coverage-matrix.md`: the three jev rows show host mechanism `native`.
- `plugins/jev/skills/jev/evals/README.md`: drop the word `bun` so the skill tree does not name it.

## Open Questions

None. Exit codes and the mandate command are decided in the brainstorm. The OpenCode `PATH` requirement is a delivery step, not an open product question.
