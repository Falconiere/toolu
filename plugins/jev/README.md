# jev

Typed judgments from TypeSafe's Jev model at runtime (skill + REST wrapper) — yes/no probabilities, choices, and scores a script can branch on.

## Install

```
/plugin install jev@toolu
```

Standalone, no dependencies.

Keep `name`, `version`, and `description` identical in the plugin's `.claude-plugin/plugin.json` and `.codex-plugin/plugin.json`. Declare `./skills/` and `./hooks/hooks.json` in the Codex manifest when those directories exist, then add matching entries to both marketplaces.

## What it provides

- **`jev` skill** (always active) — the typed-judgment protocol: when to ask Jev instead of spending a reasoning turn, and how to shape the question.
- **`jev.sh` wrapper** — `noul` (probability of yes), `choice` (pick one, with the full distribution), `score` (rate on your own ordered levels), and `ask` (many questions in one call).
- **SessionStart hook** — republishes `jev.sh` at `<config-dir>/jev/jev.sh` so the agent's shell can reach it without plugin lifecycle variables.

## Wiring

The wrapper calls TypeSafe's single evaluation endpoint,
`POST https://api.typesafe.ai/v1/systemone`, with `curl` and `jq` — no SDK.

Set `TYPESAFE_API_KEY` in your environment (keys: `https://console.typesafe.ai/settings/keys`);
it is never read from a `.env` file. `JEV_TIMEOUT` overrides the 60-second curl
timeout. Model defaults to `jev-latest`.

Full CLI reference and usage guidance: [`skills/jev/SKILL.md`](skills/jev/SKILL.md).
Plugin page: [`docs/jev/README.md`](../../docs/jev/README.md).
