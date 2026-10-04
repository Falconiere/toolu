/**
 * Legacy exemptions (`bun run check:legacy-exemptions`): re-runs oxlint, jscpd
 * and knip with every exact-path exemption lifted and fails when an exempted
 * file no longer has the finding it is exempted for, so an exemption cannot
 * outlive its cause. Exit 0 clean, 1 stale exemptions, 3 misconfigured.
 * `LEGACY_EXEMPTIONS_ROOT` points it elsewhere.
 */
import { gateMain } from "./gate-reach/gate-main.ts";
import { checkLegacy } from "./gate-reach/legacy-run.ts";

if (import.meta.main) {
  process.exitCode = gateMain("legacy-exemptions", "LEGACY_EXEMPTIONS_ROOT", checkLegacy);
}
