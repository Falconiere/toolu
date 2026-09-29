/**
 * SessionStart (#269): Codex has no plugin-dependency manifest field, so warn
 * when this plugin is installed without the toolu core, naming the exact
 * install command. Reads Codex's authoritative `codex plugin list --json`;
 * silent on Claude Code, which resolves `plugin.json` dependencies itself.
 * Port of the bash `check-toolu.sh`; never reads stdin.
 */
import { CORE_PLUGIN, codexDependencyNotice } from "@toolu/core/startup";

const notice = codexDependencyNotice([CORE_PLUGIN], "core");
if (notice !== undefined) process.stdout.write(notice);
