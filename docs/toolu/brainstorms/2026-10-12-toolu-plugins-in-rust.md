# `toolu plugins`: the installer in Rust (#438) — Brainstorm

**Date:** 2026-10-12   **Mode:** Delivery, Full path (public interface, removal of a published bin)

## Capsule

- **Outcome:** `toolu plugins install|list|remove|update` in a new crate
  `crates/plugins` (package `toolu-plugins`) does everything `tools/toolu-cli`
  does today, on claude, codex and opencode. The npm package `@toolu/plugins`
  becomes a dependency-free Node wrapper whose bin is `toolu-plugins`; it finds
  the native `toolu` (never itself) and runs `toolu plugins …`, or exits with
  the #457 install commands.
- **Material defaults / non-goals:** the TypeScript installer is deleted, its
  tests move to the Rust binary; exit codes follow the #442 contract; the live
  progress bar is dropped; nothing writes `.toolu/plugins.json` (the TS CLI never
  did); no binary download from the wrapper.
- **Repository evidence:** `tools/toolu-cli/src` (~3,000 lines, reviewed in
  full); `crates/cli/src/registry.rs` (the `planned` placeholder);
  `toolu_runtime::{cli, env, process, atomic, git, host::roots}`;
  `crates/core/protocol/src/launcher.rs` (`INSTALL_DIRS`, the `--hook-protocol`
  probe); `tooling/src/opencode-host/scenarios-cli.ts` (`cli.install`,
  `cli.lifecycle`); `include_str!` of a repository file is already used in
  `crates/pr-babysit/src/dispatch/brief.rs`.
- **Risk:** byte-exact JSONC edits; coverage (85%) of prompt glue; the many
  documents and tests that name `tools/toolu-cli/src/cli.ts` or the `toolu` bin.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|------|---------|----------|------|
| TS implementation | Delete; keep only the wrapper | Issue: the wrapper invokes the native binary; #440 removes the rest. Jev 0.93 for delete | Docs and tests that run `src/cli.ts` must move |
| Exit codes | #442 contract: usage and missing input 64, cancelled prompt 1, failure 1 | `toolu_protocol::exit` has six exits; `check-cli-compat` guards them. Jev 0.99 | A script that tested for 2/3/130 changes |
| Progress bar | Dropped; final report unchanged | Rule 14: plugin crates never write stdout/stderr; a verb returns one `Outcome`. Jev 0.67 drop, 0.31 Ctx sink | Long installs show nothing until the report |
| Prompts | Line-based numbered prompts on `/dev/tty`, logic generic over `BufRead`/`Write` | `@clack/prompts` has no Rust twin; `std::io::stdin` and `/dev/tty` are outside the stdio capability | Less pretty than clack |
| JSONC | `jsonc-parser` 0.34 (`cst`), the dprint crate the issue names, plus the TS append/remove helpers ported over its scanner tokens | MIT, no new transitive crates beyond `serde_json` | The fixtures are byte-exact |
| Catalog | The binary embeds `.claude-plugin/marketplace.json`; `--catalog <path>` overrides; the wrapper passes its bundled `assets/marketplace.json` | Acceptance keeps the npm asset | Two catalogs can differ; the wrapper's wins when it runs |
| `--config` | Implemented: install replays the file's `enabled` and `host` | The TS CLI parsed it and ignored it; the docs say "replay" | New behavior where TS had none |
| Commit type | `feat(plugins)!` | Bin rename and exit codes break documented use. Jev 0.83 | Major bump |

## Rejected

- Keeping the TS CLI beside the Rust one: two live implementations until #440.
- New `Exit` variants 3 and 130: a protocol change for one namespace.
- A `Ctx` streaming sink for progress: touches every verb's context for a cosmetic line.
- A hand-written JSONC parser: the issue names the CST crate, and its scanner
  gives the token offsets the append/remove helpers need.
