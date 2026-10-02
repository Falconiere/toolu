/**
 * The explicit status report for Codex and OpenCode status skills:
 *   TOOLU_HOST_OVERRIDE=<codex|opencode> bun <plugin>/hooks/dist/status.js [dir]
 * Reads the selected host's paths: lifecycle-only plugin variables such as
 * `PLUGIN_ROOT` never reach an ordinary skill shell call.
 */
import { collectStatus } from "./statusline/collect.ts";
import { reportText } from "./statusline/report.ts";

const dir = process.argv[2] || process.cwd();
const host = process.env.TOOLU_HOST_OVERRIDE === "opencode" ? "opencode" : "codex";
process.stdout.write(reportText(collectStatus(dir, process.env, host)));
