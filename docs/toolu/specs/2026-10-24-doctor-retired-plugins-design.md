# Doctor and `plugins remove` handle retired marketplace plugins — Design

**Date:** 2026-10-24   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** retired toolu plugins warn in doctor and can be removed with `toolu plugins remove` (#615)

## Problem
context7, exa-search, jira and agent-browser left the marketplace in the Rust rewrite (#402), but machines still have them installed at 7.11.0. Their `plugin.json` has no `hookProtocol`, so `toolu doctor` fails its skew check with "update the plugins from your host's marketplace", advice that cannot work for a plugin the marketplace no longer ships. `toolu plugins remove context7 --host claude --yes` refuses with `unknown plugin: context7`, because remove validates names against the catalog. No toolu command clears the failure.

## Non-Goals
1. `toolu plugins list` does not gain rows for retired plugins.
2. Doctor on Codex does not parse `config.toml`. `codex plugin list --json` omits plugins its marketplace no longer offers, so doctor there never saw (or failed on) them; `toolu plugins remove --host codex` still removes them.
3. No `--scope` support for `remove` on Claude Code (the adapter has none today); the retired installs seen are user-scope.
4. Muse keeps refusing names outside the catalog: `muse plugins remove <name>` is not scoped to the toolu marketplace, and Muse never had a retired toolu plugin.
5. A retired plugin that exists only in Cursor's marketplace cache (`plugins/cache/toolu/<name>/<rev>`) is Cursor's to remove (its `/plugin` UI); `toolu plugins remove --host cursor` deletes only toolu-installed `plugins/local/<name>` directories and reports the cached one `not installed`. No retired 7.x plugin was ever offered to Cursor.
6. The hook-time skew check (`crates/cli/src/hook.rs`) is unchanged; retired 7.x plugins have no native hooks.

## Architecture
**One built-in catalog.** The `include_str!` of `.claude-plugin/marketplace.json` moves from `crates/plugins/src/catalog.rs` to a new `toolu_runtime::catalog` module, with a lenient `plugin_names` reader. `toolu-plugins` keeps its validating parser over that text; `toolu doctor` (hub crate, which may not depend on the plugins crate per `layers.json`) reads only the names.

**Retired rule (doctor).** An installed toolu-marketplace plugin is retired when its name is absent from the built-in catalog and it is not newer than the binary: its manifest does not read (no valid `hookProtocol`), or `version::compare(binary, plugin)` is anything but `Older`. A plugin newer than the binary (the marketplace added it after this build) keeps today's "toolu is older; upgrade it" advice. An empty catalog (unparsable built-in text) marks nothing retired. Decided with Jev (0.98) over "any name not in the catalog", which would tell a user with an older binary to remove a new plugin.

**Skew check.** `skew_check::check(inventory, roots)` sets retired plugins aside before `skew::assess`. They never produce a mismatch or advice. Status: any mismatch fails (summary = mismatches, then the retired note); else retired or advice warns (retired note, then advice); else ok. The hint names the removal command for the host doctor inspected (Jev 0.98 over listing every host): `toolu plugins remove <names…> --host <host> --yes` on Claude Code, Codex and Cursor; on Muse, Muse's own `muse plugins remove <name>` per plugin, joined by `; ` (Jev 0.93), from `Roots::plugin_uninstall_command`. Placement in the skew check, not a new row, was chosen with Jev (0.99): the doctor check list and order stay the same.

**Remove gate.** `dispatch::remove` stops calling `install_order` for validation. Each name must be in the catalog, or, when the host is Claude Code, Codex or Cursor, be a well-formed plugin name (`Catalog::retired`): lowercase ASCII letters and digits, with single inner hyphens. Those hosts scope the removal to toolu (`<name>@toolu`, or Cursor's install marker), so a typo removes nothing else; the host's own not-found line is the failed step. Anything else stays `unknown plugin: <name>. Run \`toolu plugins list\` for the catalog.` (exit 64) and runs no host command. The check lives in `catalog/order.rs` (`order::removable`), so `dispatch.rs` (285 lines, near the 300-code-line limit) only swaps one call. Decided with Jev (0.97) over requiring each host's records to list the plugin, which needs a new Codex `config.toml` parser and a marketplace-aware Claude listing.

Reused: `version::compare`, `manifest::read`, `Roots::plugin_uninstall_command`, `Host::name`, `catalog::order`'s unknown-plugin message (extracted to one function), the existing `Step`/report rendering.

## Interfaces / Schema
- `toolu_runtime::catalog::BUILT_IN: &str` — the marketplace manifest text the binary was built with.
- `toolu_runtime::catalog::plugin_names(text: &str) -> Vec<String>` — `plugins[].name` strings in manifest order; empty when `text` is not JSON or has no `plugins` array.
- `crates/plugins`: `catalog::EMBEDDED` becomes `toolu_runtime::catalog::BUILT_IN`; `Catalog::retired(&self, name) -> bool`; `order::unknown(name) -> Fail`.
- Doctor skew check row (text): `warn skew: retired plugin context7 is no longer in the toolu marketplace` (plural: `retired plugins context7, exa-search, jira are no longer in the toolu marketplace`), then `  fix: toolu plugins remove context7 --host claude --yes`.
- Doctor skew check JSON: `details` gains `"retired": ["context7", …]` (empty array when none), names deduplicated in inventory order; `hint` carries the removal command, `null` when nothing is retired.
- `toolu plugins remove <names…> --host claude|codex|cursor --yes`: a well-formed non-catalog name runs `claude plugin uninstall <name>@toolu` / `codex plugin remove <name>@toolu` / Cursor's marker-checked delete.

## Failure modes and edge cases
- Retired plugin and a real protocol mismatch together: skew fails on the mismatch; the summary and hint still name the retired plugin.
- Same retired plugin listed twice (Claude user and project rows): named once.
- Catalog plugin whose manifest does not read: still a mismatch with the update advice, as today (it is not retired).
- Non-catalog plugin newer than the binary with a current manifest: not retired; existing advice.
- Non-catalog plugin with a readable manifest at or below the binary's version (a future retirement): retired.
- Built-in catalog unparsable: no plugin is retired; skew behaves as before.
- `remove ../x` or `remove Bad_Name` on any host, or `remove context7` on Muse: exit 64 `unknown plugin: …`, no host command runs.
- `remove nope` on Claude Code where `nope@toolu` is not installed: the host's failure line is the step, exit 1 (was exit 64).
- `remove context7 --dry-run`: prints the command, runs nothing (unchanged dry-run path).
- Mixed `remove jev context7`: each name removed independently, as today.

## Acceptance criteria
- **AC-1:** Given Claude's `installed_plugins.json` listing `context7@toolu` whose `.claude-plugin/plugin.json` is the real 7.11.0 manifest (no `hookProtocol`) beside a current `toolu@toolu`, `toolu --host claude --json doctor` exits 0, the skew check is `warn`, its summary says `retired plugin context7 is no longer in the toolu marketplace` and not `update the plugins`, its hint is `toolu plugins remove context7 --host claude --yes`, and `details.retired` is `["context7"]`.
- **AC-2:** With the `context7@toolu` entry removed from that file, the same doctor run reports skew `ok` (`plugin versions match`) and an empty `details.retired`.
- **AC-3:** A non-catalog plugin with a current `hookProtocol` and version `999.0.0` is not retired: skew warns with `is older than the <name> plugin 999.0.0`, `details.retired` is empty.
- **AC-4:** A retired plugin beside a plugin with `hookProtocol` 99 fails skew; the summary names both `hook protocol 99` and the retired plugin, and the hint is the removal command.
- **AC-5:** On Muse the retired hint is `muse plugins remove <name>` per plugin; on Codex and Cursor it is `toolu plugins remove <names> --host codex|cursor --yes`.
- **AC-6:** `toolu plugins remove context7 --host claude --yes` runs `claude plugin uninstall context7@toolu`, reports `removed`, exits 0; `--host codex` runs `codex plugin remove context7@toolu` likewise. Against the real Claude Code 2.1.310 and Codex 0.164.0 CLIs in isolated config homes holding a 7.11.0 `context7@toolu`, the command removes it (`installed_plugins.json`/`enabledPlugins`, or the `config.toml` table, no longer list it).
- **AC-8:** Running the exact hint doctor printed in AC-1 (`toolu plugins remove context7 --host claude --yes`) against a Claude CLI that drops `context7@toolu` from `installed_plugins.json` exits 0, and the next doctor run reports skew `ok`. Live: against the real Claude Code CLI in an isolated config home, doctor warns, the hinted command removes the plugin, and doctor's skew is then `ok`.
- **AC-7:** `toolu plugins remove ../x --host claude --yes` and `toolu plugins remove context7 --host muse --yes` exit 64 with `unknown plugin:` and run no host command.

## Acceptance evidence
- AC-1/AC-2/AC-4: `crates/cli/tests/doctor_report.rs`, real binary, the repo's `plugins/toolu` copied as the current plugin and a context7 root holding the captured 7.11.0 manifest text; boundary: removal of the record flips skew to ok; mismatch case uses `hookProtocol` 99.
- AC-3: `crates/toolu/src/doctor/tests/skew_check_test.rs`: Claude record for `future-plugin@toolu` whose manifest is `{"name":"future-plugin","version":"999.0.0","hookProtocol":HOOK_PROTOCOL}`; expected skew `warn`, summary contains `is older than the future-plugin plugin 999.0.0`, `details.retired == []`, no hint; boundary: the same manifest at `0.0.1` is retired.
- AC-5: `skew_check_test.rs` calls the hint builder with real `Roots` for each host and the names `["context7", "jira"]`; expected exactly `muse plugins remove context7; muse plugins remove jira`, `toolu plugins remove context7 jira --host codex --yes`, `toolu plugins remove context7 jira --host cursor --yes`; boundary: no retired names gives no hint.
- AC-6: `crates/cli/tests/plugins_hosts.rs` with the logging stub `claude`/`codex` (argv and exit); live run of the release-built binary against the real CLIs in `/private/tmp` homes, recorded in `docs/toolu/evidence/2026-10-24-doctor-retired-plugins.md`.
- AC-8: `crates/cli/tests/doctor_report.rs` reads the hint from doctor's JSON, runs it with the real binary against a stub `claude` whose `plugin uninstall context7@toolu` rewrites `installed_plugins.json` without that entry (what the real CLI does, verified live), then reruns doctor; boundary: the stub log shows exactly one `plugin uninstall context7@toolu`. Live: the same sequence with the real CLI (`CLAUDE_CONFIG_DIR` in `/private/tmp`), recorded in the evidence file.
- AC-7: `crates/cli/tests/plugins_hosts.rs` (stub logs stay empty) plus `crates/plugins/src/tests/catalog_test.rs` for `Catalog::retired` boundaries (`context7`, `exa-search`, `jev` in catalog → false, `../x`, `Bad`, `-x`, `x-`, `a--b`, empty → false).
- Built-in catalog: `crates/core/runtime/src/tests/catalog_test.rs` (`BUILT_IN` names include `toolu` and `jev`, exclude `context7`; malformed text → empty).

## Documentation impact
- `docs/install.md` "Version skew": a retired plugin warns with its removal command instead of counting as a mismatch.
- `docs/cli/installer.md`: `remove` accepts a plugin the marketplace retired on Claude Code, Codex and Cursor; Muse refuses names outside the catalog.
- Generated `docs/cli/*.md` and the CLI contract: no help text changes; `cargo xtask docs-cli` must stay clean.

## Open Questions
None. Interpretations recorded above: "removal command for each host" means the inspected host's command (Jev 0.98); Muse uses its native remove command (Jev 0.93).
