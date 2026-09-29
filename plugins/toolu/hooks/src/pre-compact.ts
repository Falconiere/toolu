/**
 * PreCompact (#263): toolu has nothing to say before a compaction, so the hook
 * drains stdin (Codex sees EPIPE from a hook that exits before reading) and
 * exits 0 silently, enabled or not. Port of the bash `pre-compact.sh`.
 */
import { loadConfig } from "@toolu/core/config";
import { detectHost } from "@toolu/core/host";

try {
  // Loaded for its config warnings, exactly as the bash hook sourced config.sh.
  loadConfig({ env: process.env, host: detectHost({ env: process.env }) });
  await Bun.stdin.text();
} catch (error) {
  process.stderr.write(`toolu pre-compact: ${String(error)}\n`);
}
