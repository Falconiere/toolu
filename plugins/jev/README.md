# jev

Typed judgments from TypeSafe's Jev model at runtime (skill + REST wrapper) — yes/no probabilities, choices, and scores a script can branch on.

## Install

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
need `curl`, `jq`, and `TYPESAFE_API_KEY` in their launch environment.

Standalone, no plugin dependencies.

## What it provides

- **`jev` skill** — mandatory when semantic decisions exist: gather evidence, call before the decision it informs, and reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions and reuse unchanged results.
- **`jev.sh` wrapper** — `noul` (probability of yes), `choice` (pick one, with the full distribution), `score` (rate on your own ordered levels), and `ask` (many questions in one call).
- **SessionStart hook** — publishes `jev.sh` at `<config-dir>/jev/jev.sh` and injects the full mandate on startup, resume, clear, and compaction. It checks local prerequisites and makes no API call.
- **UserPromptSubmit hook** — restates the mandate on every prompt so it survives long sessions; silent for trivial confirmations and when prerequisites are missing (SessionStart already reported them). No API call.

## Wiring

The wrapper calls TypeSafe's single evaluation endpoint,
`POST https://api.typesafe.ai/v1/systemone`, with `curl` and `jq` — no SDK.

Set `TYPESAFE_API_KEY` in your environment (keys: `https://console.typesafe.ai/settings/keys`);
it is never read from a `.env` file. `JEV_TIMEOUT` overrides the 60-second timeout
per attempt. Model defaults to `jev-latest`.

Full CLI reference and usage guidance: [`skills/jev/SKILL.md`](skills/jev/SKILL.md).
Executable [problem-solving examples](skills/jev/references/problem-solving.md) cover search, debugging, planning, and review with uncertainty and no-match handling.
Plugin page: [`docs/jev/README.md`](../../docs/jev/README.md).
