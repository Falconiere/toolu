# Epic engine config and secrets — Design

**Date:** 2026-10-06   **Status:** Approved   **Author:** epic worker (Codex)   **Topic:** #463, shared settings, secrets, redaction and status token rotation.

Brainstorm: `docs/toolu/brainstorms/2026-10-06-epic-config-secrets.md`.

## Problem

The future status server, notifications and peer clients need credentials, but project `toolu.config.json` is commonly committed. The Rust and TypeScript loaders coexist, and a new top-level key must not block TypeScript hooks. There is no common secret reader or safe output contract yet.

## Non-Goals

1. #445 owns `toolu config get` and the expanded `doctor`; #434 owns the journal; #449 owns the status API and page. This issue provides their shared reader and redactor, and verifies current CLI output plus redacted representative documents. Those issues must add their own integration tests when their surfaces land.
2. #449, #450 and #453 own listener, notification and fleet behavior. This change defines their settings and secret inputs without implementing their network operations.
3. No migration of old credentials from project config: there is no existing engine setting to migrate.

## Architecture

`toolu-runtime::config::epic` interprets the existing open `epic` namespace in the merged `LoadedConfig`. `toolu-runtime::config::secrets` is the only reader and writer of `<Roots::config_root()>/toolu/secrets.json`. The epic plugin exposes `toolu epic token new`, uses the shared store, and keeps its other placeholder verbs. Runtime supplies a redacted JSON/text view for every later presenter. It never exposes a secret through `Debug`, errors or command output. Consumers reread the file for each authentication decision, so token rotation takes effect immediately.

The storage file is strict on `version` and known value types, but preserves unknown fields during rotation for forward compatibility. Reads use a no-follow open and inspect the opened file's mode. Writes create a fresh `0600` file beside it and rename atomically; an existing unsafe file or symlink is rejected. The status token is 32 random bytes from the OS, hex encoded. No token value appears on stdout/stderr, including under `--json`.

## Interfaces / Schema

`toolu.config.json` permits `epic` as an open object in both loaders (#414 already supplies this). The semantic reader accepts `epic.http.bind` (string, default `127.0.0.1`), `epic.http.port` (integer, default `7717`), `epic.attention.enabled` (boolean, default `false`) and `epic.peers` (array of `{name, url}` with unique nonempty names and credential-free HTTP(S) origins). A peer URL may have a host and port but no user info, path beyond `/`, query or fragment; the `url` crate parses it. Unknown `epic` keys are retained and ignored for binary skew. A key whose name denotes a credential (`token`, `secret`, `password`, `notify_url`/`notifyUrl`) is rejected anywhere under `epic`; notification URLs always use the secret source because credentials can hide in their path or query.

`<config>/toolu/secrets.json`:

```json
{"version":1,"status_token":"...","notify_url":"...","peer_tokens":{"server":"..."}}
```

Every value is optional until a consumer requires it. `TOOLU_EPIC_STATUS_TOKEN` overrides `status_token`; `TOOLU_EPIC_TOKEN` is a compatibility alias used only when the primary is unset; `TOOLU_EPIC_NOTIFY_URL` overrides `notify_url`; `TOOLU_EPIC_PEER_TOKENS` is a JSON object whose entries override file entries by peer name. Empty environment values count as unset. `Secrets` has accessors for these three fields, a redacted `Debug`, and `redact_json`/`redact_text` helpers. The redactor replaces credential fields and any loaded credential substring with the exact string `<redacted>`. Errors name the failure and `chmod 600` for unsafe mode but never quote file or environment contents.

`toolu epic token new` rotates only `status_token`, preserving the other file fields; it reports the file path and success. It refuses to rotate while either status-token environment override is set, because the command cannot change its parent environment and a successful rotation must invalidate the effective old token. `--json` emits one document whose only token value is `"<redacted>"`.

## Failure modes and edge cases

- An absent secrets file yields no file values; env values still work. A malformed JSON file, non-object, bad version or wrong value type fails closed, including when an environment override exists. A `0644` file is refused with `chmod 600` guidance. A symlink is refused. A permission or random-source failure returns a sanitized error.
- A config with an unknown top-level key still fails closed. A missing `epic` section uses defaults; a wrong-typed section or setting, duplicate peer name, peer URL with user info/path/query/fragment, or secret-looking setting fails with a path-only error; an unknown non-secret nested key is ignored.
- Two token rotations each atomically replace the file; the last completed rename wins. Readers see a complete old or new document. Rotations never silently repair an unsafe file and never emit either token. With `TOOLU_EPIC_STATUS_TOKEN` or `TOOLU_EPIC_TOKEN` set, rotation fails before changing the file and names the variable, not its value.
- Peer environment JSON must be an object of nonempty string tokens; malformed entries fail closed. The primary token env var takes precedence over the alias even when both are present.
- Redaction replaces a secret inside longer text (including a URL or panic message). It also redacts known credential field names even when the file cannot be loaded. Redaction does not make an arbitrary caller safe if it deliberately bypasses this API; future presenter issues must wire it and test that wiring.

## Acceptance criteria

- **AC-1:** A real `toolu.config.json` with `epic` and unknown nested settings loads in both TypeScript and Rust, while the same file with an unknown top-level key fails closed in both.
- **AC-2:** A real `0644` secrets file is refused with a `chmod 600` message; a `0600` file loads, and symlink and malformed files fail without printing canary bytes.
- **AC-3:** With file and environment values set, the status and notification values from the environment win; each peer token is overridden by the matching environment entry, and an unset entry uses the file value.
- **AC-4:** Running `toolu epic token new` creates a `0600` file with a 64-character random hex token, and a second run changes it while preserving notification and peer fields. Human and JSON output show no token; with an active status-token environment override, the command refuses and keeps the file unchanged.
- **AC-5:** A canary in all three secret fields is absent from redacted representative config, doctor, journal and status JSON/text documents, including when embedded inside longer strings; each secret field renders `<redacted>`.
- **AC-6:** The typed `epic` settings reader resolves valid non-secret values and defaults, rejects credentials in config (including a token embedded in a peer URL) and bad typed values, and ignores unknown safe nested keys for older-binary compatibility.
- **AC-7:** The changed crates pass the Rust gate and the TypeScript gate with no exemption; generated CLI docs and command snapshot match the binary.

## Acceptance evidence

| AC | Real input and expected result | Boundary | Runnable check |
|---|---|---|---|
| AC-1 | the same shared fixture cases staged as temporary config files for the TypeScript and Rust loaders, valid `epic` then invalid sibling | unknown nested versus top-level | `bun test packages/toolu-core/src/config/__tests__/config-fixture.test.ts`; `cargo test -p toolu-runtime --test config_fixture` |
| AC-2 | temporary files with Unix modes `0600`, `0644`, a symlink and malformed bytes; safe load or sanitized error | bad permissions, symlink, malformed | `cargo test -p toolu-runtime --lib -- config::secrets::` |
| AC-3 | a real temp file and an explicit `Env` snapshot, resolved accessors | primary/alias and per-peer precedence, malformed env JSON | `cargo test -p toolu-runtime --lib -- config::secrets::` |
| AC-4 | real `toolu epic token new` subprocess twice against a temp config dir; complete JSON file and no token in either output | rotation, `--json`, unsafe existing file, active environment override | `cargo test -p toolu-cli --test epic_token` and epic plugin unit tests |
| AC-5 | four representative serialized documents with sentinel values, run through the shared redactor; current command output is checked too | embedded string and named secret key | `cargo test -p toolu-runtime --lib -- config::secrets::`; `cargo test -p toolu-cli --test epic_token` |
| AC-6 | `LoadedConfig::from_data` with defaults, valid settings and rejected credential keys | bad type, duplicate peer, token in URL path or query, unknown nested safe field | `cargo test -p toolu-runtime --lib -- config::epic::` |
| AC-7 | the branch and generated command reference | quality, drift, coverage | `cargo xtask gate --base origin/main`; `bun run test` |

## Documentation impact

Update `docs/config.md` with the settings and secret file/environment contract, and regenerate `docs/cli/epic.md` and command snapshots from the binary. Record the runtime module and epic verb in `AGENTS.md` only if the key-file summary needs to change.

## Open Questions

None blocking. #449, #450 and #453 must consume this module and add their output integration tests; #445 and #434 have the same obligation for management output and journal records. The representative redactor test in this PR establishes the shared serializer contract but cannot prove those later presenters call it. Each owning issue must plant a secret canary in a real file/environment and assert its actual output omits it; this is a recorded residual risk, not an acceptance claim for those unimplemented surfaces.

## Spec review

Round 1: Jev rated implementable issue coverage 1.88/2 (0.82 confidence) and identified future-output wiring as the leading risk (0.83). Added explicit residual risk and owner integration checks. The agent also found that a peer URL could conceal a token in its path or query; constrained it to a credential-free origin. No remaining blocker. **Status:** Approved.

Execution clarification: after testing env precedence, a file rotation under an active status env override would leave the effective old token valid. Jev favored refusing rotation in that state (0.94); the interface, failure case and AC-4 now require that refusal. No other AC changed. **Status:** Approved.
