// @bun
// plugins/epic-orchestrator/hooks/src/engine-ensure.ts
import { spawnSync } from "child_process";
var PROBE_MS = 1000;
var ENSURE_MS = 5000;
function shellToolu() {
  const found = spawnSync("/bin/sh", ["-c", "command -v toolu"], {
    timeout: PROBE_MS,
    encoding: "utf8"
  });
  if (found.status !== 0)
    return;
  const text = found.stdout.trim();
  return text.startsWith("/") ? text : undefined;
}
function native(path) {
  const result = spawnSync(path, ["--hook-protocol"], { timeout: PROBE_MS, encoding: "utf8" });
  return result.status === 0 && /^[1-9][0-9]*$/u.test(result.stdout.trim());
}
await Bun.stdin.text();
var candidate = process.env["TOOLU_BIN"] ?? shellToolu();
if (candidate !== undefined && native(candidate)) {
  const ran = spawnSync(candidate, ["epic", "engine", "--ensure"], {
    timeout: ENSURE_MS,
    encoding: "utf8"
  });
  if (ran.status !== 0) {
    const detail = (ran.stderr || ran.stdout || "ensure failed").trim().slice(0, 200);
    process.stdout.write(`${JSON.stringify({ systemMessage: `epic engine: ${detail}` })}
`);
  }
}
