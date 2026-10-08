/** SessionStart: start the resident engine when a native `toolu` is available. */
import { spawnSync } from "node:child_process";

const PROBE_MS = 1_000;
const ENSURE_MS = 5_000;

function shellToolu(): string | undefined {
  const found = spawnSync("/bin/sh", ["-c", "command -v toolu"], {
    timeout: PROBE_MS,
    encoding: "utf8",
  });
  if (found.status !== 0) return undefined;
  const text = found.stdout.trim();
  return text.startsWith("/") ? text : undefined;
}

function native(path: string): boolean {
  const result = spawnSync(path, ["--hook-protocol"], { timeout: PROBE_MS, encoding: "utf8" });
  return result.status === 0 && /^[1-9][0-9]*$/u.test(result.stdout.trim());
}

function absolute(path: string): string | undefined {
  return path.startsWith("/") ? path : undefined;
}

await Bun.stdin.text();
const fromEnv = process.env["TOOLU_BIN"];
const candidate = fromEnv === undefined ? shellToolu() : absolute(fromEnv);
if (candidate !== undefined && native(candidate)) {
  const ran = spawnSync(candidate, ["epic", "engine", "--ensure"], {
    timeout: ENSURE_MS,
    encoding: "utf8",
  });
  if (ran.status !== 0) {
    const detail = (ran.stderr || ran.stdout || "ensure failed").trim().slice(0, 200);
    process.stdout.write(`${JSON.stringify({ systemMessage: `epic engine: ${detail}` })}\n`);
  }
}
