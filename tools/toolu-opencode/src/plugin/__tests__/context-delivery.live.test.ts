/**
 * Live model capture (#341, AC-6). Skipped unless `TOOLU_LIVE_OPENCODE=1`,
 * so the ledger check stays hermetic. This delivery runs the same file once
 * with the flag and records the capture.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { runHost, withServe } from "../../../../../tooling/src/opencode-host/host-run.ts";
import {
  hostCacheDir,
  resolveHostBinary,
} from "../../../../../tooling/src/opencode-host/install.ts";
import { contractPaths } from "../../../../../tooling/src/opencode-host/results.ts";
import {
  allRequestText,
  messagesText,
  toolRequestCount,
} from "../../../../../tooling/src/opencode-host/scenario.ts";
import { summarize } from "../../../../../tooling/src/opencode-host/scenarios-context.ts";
import {
  PROJECT_FILES,
  ROOT,
  diagnostics,
  installShim,
} from "../../../../../tooling/src/opencode-host/scenarios-entry.ts";
import { PinSchema, readJson } from "../../../../../tooling/src/opencode-host/schema.ts";
import { openSession } from "../../../../../tooling/src/opencode-host/session.ts";

const PROMPT = "rename the parser in src/parser.ts";
const RENAME = "Rename: find all refs";
const RECOVER = "Recover memories before continuing";

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "the pinned host sends startup, prompt and compaction context to the model",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    using session = openSession(cacheRoot, {
      config: () => ({ permission: { bash: "allow" } }),
      files: PROJECT_FILES,
    });
    installShim(session);
    session.env.TOOLU_BUN = process.execPath;
    session.env.TOOLU_REPO_ROOT = ROOT;
    const hostRun = await runHost(host.bin, session, ["--print-logs", PROMPT]);
    expect(hostRun.exitCode).toBe(0);
    expect(diagnostics(hostRun.stderr, "toolu: ready")).toBe(1);
    expect(diagnostics(hostRun.stderr, "toolu: not ready")).toBe(0);
    expect(toolRequestCount(session)).toBeGreaterThan(0);
    const system = messagesText(session, "system");
    const user = messagesText(session, "user");
    expect(system).toContain("Session Protocol");
    expect(user).toContain(PROMPT);
    expect(user).toContain(RENAME);
    const summarized = await withServe(host.bin, session, (url) =>
      summarize(url, session.sb.project),
    );
    expect(summarized).toBe(true);
    const captured = allRequestText(session);
    expect(captured).toContain("toolu sessionID:");
    expect(captured).toContain(RECOVER);
    process.stdout.write(
      `${JSON.stringify({
        host: host.version,
        system: system.includes("Session Protocol"),
        prompt: user.includes(PROMPT),
        rename: user.includes(RENAME),
        sessionId: captured.includes("toolu sessionID:"),
        recover: captured.includes(RECOVER),
      })}\n`,
    );
  },
  600_000,
);
