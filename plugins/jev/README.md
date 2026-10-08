# jev

Typed judgments from TypeSafe's Jev model at runtime (skill + REST wrapper) — yes/no probabilities, choices, and scores a script can branch on.

## Install

**Prerequisite:** the native `toolu` binary. See [docs/install.md](../../docs/install.md).

Claude Code:

```text
/plugin marketplace add Falconiere/toolu
/plugin install jev@toolu
```

Codex:

```bash
codex plugin marketplace add Falconiere/toolu
codex plugin add jev@toolu
```

Restart the host after installation. In Codex, review and trust the installed
hook through `/hooks`; installation alone does not trust hooks. Both hosts
need `TYPESAFE_API_KEY` in the environment executing Jev. Hook and command
environments can differ; an absent hook key requires checking the command
environment without printing the value before declaring Jev unavailable.

OpenCode: add `jev` to `.opencode/toolu/plugins.json` (see [docs/opencode.md](../../docs/opencode.md)).

- The wrapper is published at `$TOOLU_CONFIG_DIR/jev/jev.sh`, the project's data root that `shell.env` gives every bash call.
- The skill is listed as `jev-jev`.
- The mandate is in every request's system prompt, and each substantive prompt gets one reminder. Compaction only relinks the wrapper.
- The command is `toolu jev` and does not load a project `.env` file.

Standalone, no plugin dependencies.

## What it provides

- **`jev` skill** — mandatory when semantic decisions exist: gather evidence, call before the decision it informs, and reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions and reuse unchanged results.
- **Jev CLI** — `toolu jev` provides `noul` (probability of yes), `choice` (pick one, with the full distribution), `score` (rate on your own ordered levels), and `ask` (many questions in one call).
- **SessionStart hook** — publishes `<config-dir>/jev/jev.sh` and injects the mandate to run `toolu jev` on startup, resume, clear, and compaction. It makes no API call.
- **UserPromptSubmit hook** — restates that mandate on substantive prompts so it survives long sessions; silent for trivial confirmations and unpublished wrappers. An absent hook key prompts command-environment verification. No API call.

## Wiring

`toolu jev` calls TypeSafe's single evaluation endpoint,
`POST https://api.typesafe.ai/v1/systemone`.

Set `TYPESAFE_API_KEY` in your environment (keys: `https://console.typesafe.ai/settings/keys`);
it is never read from a `.env` file. `JEV_TIMEOUT` overrides the 60-second timeout
per attempt. Model defaults to `jev-latest`.

Full CLI reference and usage guidance: [`skills/jev/SKILL.md`](skills/jev/SKILL.md).
Executable [problem-solving examples](skills/jev/references/problem-solving.md) cover search, debugging, planning, and review with uncertainty and no-match handling.
Plugin page: [`docs/jev/README.md`](../../docs/jev/README.md).

A regular file already at `jev.sh` is a user-provided executable override. It is
preserved and invoked directly using its own shebang/interpreter; this supports
both shell scripts and JavaScript executables. Raw JavaScript without an
executable shebang is not an executable override. The published link means
`toolu jev`.

## Wrapper reference

Text input only; preprocess other formats. Context: 64k tokens/request, 32k for
state + longest question. Slice large inputs. English is the primary training language.

`@FILE`/stdin become structured state only for a JSON object/array; other values
stay strings. Default output is `.answers`; `--raw` includes model/token usage.
Missing/invalid answers fail explicitly.

Retries: at most three attempts for timeout, connection failure, HTTP 408/429/5xx;
1s then 2s backoff. `Retry-After` seconds or `retry-after-ms` up to 60s is honored;
longer waits surface the error. Other 4xx, including 401/422, are not retried.

Exit codes: `1` failure (including an HTTP body on stderr); `64` usage;
`75` timeout. `JEV_TIMEOUT` sets the timeout per attempt (default 60s).
