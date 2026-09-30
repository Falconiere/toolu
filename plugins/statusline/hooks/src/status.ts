/**
 * The explicit status report for Codex's `$statusline:status` skill:
 *   TOOLU_HOST_OVERRIDE=codex bun <plugin>/hooks/dist/status.js [dir]
 * Always reads Codex paths: lifecycle-only plugin variables such as
 * `PLUGIN_ROOT` never reach an ordinary skill shell call.
 */
import { collectStatus } from "./statusline/collect.ts";
import { reportText } from "./statusline/report.ts";

const dir = process.argv[2] || process.cwd();
process.stdout.write(reportText(collectStatus(dir, process.env, "codex")));
