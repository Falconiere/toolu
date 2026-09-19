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

- **`jev` skill** — mandatory for useful, bounded semantic decisions over supplied evidence, with batching and explicit fallback.
- **`jev.sh` wrapper** — `noul` (probability of yes), `choice` (pick one, with the full distribution), `score` (rate on your own ordered levels), and `ask` (many questions in one call).
- **SessionStart hook** — publishes `jev.sh` at `<config-dir>/jev/jev.sh` and injects a short mandatory workflow on startup, resume, clear, and compaction. It checks local prerequisites and makes no API call.

## Wiring

The wrapper calls TypeSafe's single evaluation endpoint,
`POST https://api.typesafe.ai/v1/systemone`, with `curl` and `jq` — no SDK.

Set `TYPESAFE_API_KEY` in your environment (keys: `https://console.typesafe.ai/settings/keys`);
it is never read from a `.env` file. `JEV_TIMEOUT` overrides the 60-second timeout
per attempt. Model defaults to `jev-latest`.

Full CLI reference and usage guidance: [`skills/jev/SKILL.md`](skills/jev/SKILL.md).
Plugin page: [`docs/jev/README.md`](../../docs/jev/README.md).
