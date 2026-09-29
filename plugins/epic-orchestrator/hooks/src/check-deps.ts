/**
 * SessionStart (#269): Codex has no plugin-dependency manifest field, so warn
 * when epic-orchestrator is installed without delivery-flow and the plugins
 * it drives, naming each exact install command. Reads Codex's authoritative
 * `codex plugin list --json`; silent on Claude Code, which resolves
 * `plugin.json` dependencies itself. Port of the bash `check-deps.sh`.
 */
import { CORE_PLUGIN, codexDependencyNotice } from "@toolu/core/startup";

const REQUIRED = [
  CORE_PLUGIN,
  "delivery-flow@toolu",
  "toolu-review@toolu",
  "pr-babysit@toolu",
  "brainstorm@toolu",
];

const notice = codexDependencyNotice(REQUIRED, "each");
if (notice !== undefined) process.stdout.write(notice);
