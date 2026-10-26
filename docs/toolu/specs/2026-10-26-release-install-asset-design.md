# Release install.sh asset for get.toolu.sh — Design

**Date:** 2026-10-26   **Status:** Approved   **Author:** Falconiere Barbosa (with Claude)   **Topic:** Every toolu release uploads `install.sh`, so get.toolu.sh can redirect to `releases/latest/download/install.sh` instead of a hand-moved commit pin.

## Problem

`get.toolu.sh/pkg/toolu/install` 302-redirects to `install.sh` at a commit pinned in `CodaSignal/toolu.sh`. After the 2026-10-26 key rotation, that pin served an installer that trusted only the lost key. Publishing v8.4.1 would therefore have broken the one-liner for everyone until someone moved the pin by hand. The user chose a dynamic redirect: the worker points at `https://github.com/Falconiere/toolu/releases/latest/download/install.sh`, which GitHub resolves to the newest stable release, never a draft or prerelease. That only works if every release carries `install.sh` as an asset.

## Non-Goals

1. The worker change itself, which ships in `CodaSignal/toolu.sh` (its own PR and deploy).
2. Adding `install.sh` to `SHA256SUMS`. `curl | bash` users cannot verify it before running it.
3. Backfilling releases before v8.4.0. Only `latest` is served. v8.4.0 already has the asset; it was uploaded by hand on 2026-10-26.

## Architecture

Brainstorm, compact: the outcome is that the served installer always comes from the latest release. The default is to upload `install.sh` from the tagged commit. Jev chose the release-asset redirect over a request-time tag API (0.99). The accepted risk: publishing a release now changes the installer directly, with no second repository involved.

- `release-native.yml` `package`: copy `install.sh` from the tagged checkout into `dist/`. `upload` adds `dist/install.sh` to `gh release upload`. The existing round trip (`for file in dist/*; do cmp …`) then proves the draft holds it.
- `crates/xtask/src/workflow_install.rs` `signing()`: one more `found.need`. Its message is "release-native.yml must upload install.sh, which get.toolu.sh serves from the latest release". If a later edit drops the asset, `cargo xtask check-workflows` fails.
- `.claude/skills/deploy/SKILL.md`:
  - Step 6 copies `install.sh` into `$D`.
  - Step 8 uploads it.
  - Step 10 checks that `releases/latest/download/install.sh` and `get.toolu.sh/pkg/toolu/install` both serve bytes identical to `install.sh`.

## Interfaces / Schema

Release assets: the four `toolu-<os>-<arch>.tar.gz`, `SHA256SUMS`, `SHA256SUMS.minisig`, `toolu.spdx.json` (CI only), and **`install.sh`**.

## Failure modes and edge cases

- **CI workflow missing the asset:** `check-workflows` fails with the new message.
- **Upload or round-trip failure:** the release stays a draft (existing behavior), so `latest` and the served installer stay on the previous release.
- **Draft or prerelease:** `releases/latest` ignores it, so the served installer changes only when a stable release is published.
- **Step-10 mismatch:** for example, the worker still pins a commit. `/deploy` reports it as a failure. The release is already public, so the report names the worker change as the fix.

## Acceptance criteria

- **AC-1:** `install_channels` over an empty root reports "release-native.yml must upload install.sh, which get.toolu.sh serves from the latest release", and `cargo xtask check-workflows` on this repository reports none.
- **AC-2:** `release-native.yml` copies `install.sh` into `dist/` in `package` and uploads `dist/install.sh` in `upload`.
- **AC-3:** The `/deploy` skill copies and uploads `install.sh`, and its step 10 compares both served URLs with `install.sh`.
- **AC-4:** `docs/releases/native.md` lists `install.sh` among the release assets and describes the get.toolu.sh redirect, replacing the "pin must move" follow-up.
- **AC-5 (deferred, live):** After `/deploy --env prod --tag v8.4.1`, `curl -fsSL https://get.toolu.sh/pkg/toolu/install` is byte-identical to v8.4.1's `install.sh`.
- **AC-6:** `cargo xtask gate --only` passes every step except `tests` and `coverage`, plus `cargo test -p xtask workflow_install`.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
| --- | --- | --- | --- | --- |
| AC-1 | Empty temp root; this repo's workflows | Message present; no findings | Upload line without `install.sh` | `cargo test -p xtask workflow_install::tests`; `cargo xtask check-workflows` |
| AC-2 | `.github/workflows/release-native.yml` | Copy and upload lines present | — | `cargo xtask check-workflows` |
| AC-3 | `.claude/skills/deploy/SKILL.md` | Lines present | Step 10 fails on mismatch | `grep` |
| AC-4 | `docs/releases/native.md` | Text present | — | `grep` and `cargo xtask check-markdown-cli` |
| AC-5 | The live v8.4.1 release and the deployed worker | `cmp` exit 0 | Worker not yet switched | `curl -fsSL https://get.toolu.sh/pkg/toolu/install \| cmp - <(gh release download v8.4.1 -p install.sh -O -)` |
| AC-6 | The final tree | Exit 0 | Host-only steps excluded | Gate command |

## Documentation impact

`docs/releases/native.md` (assets, the get.toolu.sh redirect, the rotation follow-up) and the `/deploy` skill.

## Open Questions

None blocking. The worker change ships in `CodaSignal/toolu.sh`, owned by this delivery; its prod deploy needs the user's go-ahead.

## Spec review (2026-10-26)

- Acceptance criteria: 🔵 consider: AC-5 can be proven only after the worker deploy and the first release. It is marked deferred, and AC-3's step 10 performs the same comparison during the run.
- No blockers. Every AC has a runnable check on real files or the live release.
