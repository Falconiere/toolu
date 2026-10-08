# Jev in Rust — Brainstorm

**Date:** 2026-10-08
**Issue:** Falconiere/toolu#430
**Mode:** Delivery, full path

## Capsule

- **Outcome:** `toolu jev noul|choice|score|ask` runs on the core Jev client. SessionStart publishes a one-line `jev.sh` shim and injects a mandate that tells the agent to run `toolu jev`. UserPromptSubmit repeats that reminder except for trivial prompts. `plugins/jev` has no Bun sources.
- **Material defaults / non-goal:** Exit statuses are the six `toolu` codes (0, 1, 2, 64, 69, 75). Curl statuses 22 and 28 are not kept. Help text is clap's, generated into `docs/cli/`. The shim stays for old callers until #440. This issue does not remove Bun from other plugins.
- **Repository evidence:** `crates/jev` is the planned namespace. `toolu-jev-client` already speaks the TypeSafe API. `crates/cli` exits only through `toolu_protocol::exit::Exit`. `startup::publish` already keeps a non-symlink at `jev.sh`.
- **Risk:** OpenCode and TypeScript bootstrap tests execute jev's real SessionStart. After `hooks/dist` is gone they need a built `toolu` on `PATH`.
- **Handoff:** spec

## Axes

- **Intent.** Port the jev plugin off Bun onto `toolu jev` and the native launcher, keeping the judgment output and the missing-key message.
- **Interface.** Clap verbs match the flags in `parse.ts`. Success prints the compact answers object. `--raw` prints the pretty reply. A missing key prints `jev: TYPESAFE_API_KEY unset` and no judgment.
- **Failure.** Jev (2026-10-08, choice `contract`, confidence 1): map HTTP and timeout onto the existing exit enum. A terminal HTTP error prints the raw body and exits 1. A timeout exits 75. Bad argv is clap usage, exit 64.
- **Integration.** Hooks live in `toolu-jev` and `crates/cli` dispatches them. Stdout for those hooks is `hookSpecificOutput`, not `systemMessage`. The published file is `plugins/jev/scripts/jev.sh` (`exec toolu jev "$@"`). A kept user file is still named by its quoted path. Jev (choice `namespace`, confidence 0.34, probabilities namespace 0.67 / shim 0.33): the mandate for our own shim says `toolu jev`.
- **Constraints.** The plugin crate does not write stdout or stderr. The API key is never printed. Loopback tests use `toolu_http_test_support::Fixture` through `Config`, the same way the core client does.
- **Horizon.** `check-binary` is ported with the other two hooks so deleting `hooks/src` does not drop the native-binary notice. A shared runtime helper is the home for that notice; later plugins can call it.

## Rejected

- Preserving process statuses 22 and 28. `main` cannot return them without leaving the CLI contract.
- Telling the agent to run the shim path. The shim is the compatibility path for old callers; the skill and the mandate name `toolu jev`.
- Keeping `hooks/dist` as a Bun fallback. The issue deletes it.
