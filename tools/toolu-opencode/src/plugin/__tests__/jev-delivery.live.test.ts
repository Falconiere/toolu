/**
 * Live Jev proof (#350, AC-6), reported apart from the hermetic suite.
 * `TOOLU_LIVE_OPENCODE=1` drives the pinned host: the model sees the mandate
 * and the reminder, and its bash runs the published wrapper against the
 * loopback fixture. `TOOLU_LIVE_JEV=1` makes one real TypeSafe call through
 * the same wrapper and `shell.env` environment, and fails without a key.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { runHost, toolStates } from "../../../../../tooling/src/opencode-host/host-run.ts";
import {
  hostCacheDir,
  resolveHostBinary,
} from "../../../../../tooling/src/opencode-host/install.ts";
import { contractPaths } from "../../../../../tooling/src/opencode-host/results.ts";
import { allRequestText, messagesText } from "../../../../../tooling/src/opencode-host/scenario.ts";
import {
  PROJECT_FILES,
  ROOT,
  diagnostics,
  installShim,
} from "../../../../../tooling/src/opencode-host/scenarios-entry.ts";
import { PinSchema, readJson } from "../../../../../tooling/src/opencode-host/schema.ts";
import { openSession } from "../../../../../tooling/src/opencode-host/session.ts";
import { createTooluHooks } from "../hooks.ts";
import {
  ANSWER,
  KEY,
  MANDATE,
  REMINDER,
  SECRET,
  SELECTION,
  bashEnv,
  binding,
  calledCommand,
  inShell,
  selectJev,
  systemLines,
} from "./jev-fixtures.ts";

const PROMPT = "PROBE:jev.judge rank these two parser designs";

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "the pinned host gives the model Jev's mandate and reminder, and its bash runs a typed judgment",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    const fixture = await startHttpsFixture(["api.typesafe.ai"]);
    const proxy = Object.entries(fixture.env)
      .map(([name, value]) => `${name}='${value}'`)
      .join(" ");
    const judge = `${proxy} toolu jev noul probe -s evidence`;
    try {
      using session = openSession(cacheRoot, {
        config: () => ({ permission: { bash: "allow" } }),
        files: {
          ...PROJECT_FILES,
          ".env": `TYPESAFE_API_KEY=${SECRET}\n`,
          ".opencode/toolu/plugins.json": SELECTION,
        },
        scripts: { "jev.judge": [{ tool: "bash", args: { command: judge, description: "jev" } }] },
      });
      installShim(session);
      session.env.PATH = `${join(ROOT, "target/debug")}:${process.env.PATH ?? ""}`;
      session.env.TOOLU_BUN = process.execPath;
      session.env.TOOLU_REPO_ROOT = ROOT;
      session.env.TYPESAFE_API_KEY = KEY;
      fixture.plan([{ body: JSON.stringify(ANSWER) }]);
      const hostRun = await runHost(host.bin, session, ["--print-logs", PROMPT]);
      expect(hostRun.exitCode).toBe(0);
      expect(diagnostics(hostRun.stderr, "toolu: ready")).toBe(1);
      const system = messagesText(session, "system");
      const user = messagesText(session, "user");
      expect(system).toContain(MANDATE);
      expect(system).toContain("toolu jev");
      expect(system).toContain("jev-jev");
      expect(user).toContain(REMINDER);
      expect(toolStates(hostRun)).toEqual([{ tool: "bash", status: "completed", error: null }]);
      expect(fixture.requests).toHaveLength(1);
      expect(fixture.requests[0]?.headers.authorization).toBe(`Bearer ${KEY}`);
      const captured = allRequestText(session);
      expect(captured).toContain("0.92");
      expect(captured).not.toContain(SECRET);
      process.stdout.write(
        `${JSON.stringify({
          host: host.version,
          mandate: system.includes(MANDATE),
          reminder: user.includes(REMINDER),
          judgment: captured.includes("0.92"),
          fixtureRequests: fixture.requests.length,
          dotenvSeen: captured.includes(SECRET),
        })}\n`,
      );
    } finally {
      await fixture.stop();
    }
  },
  600_000,
);

test.skipIf(process.env.TOOLU_LIVE_JEV !== "1")(
  "one real TypeSafe judgment runs through the OpenCode-published wrapper",
  async () => {
    const key = process.env.TYPESAFE_API_KEY ?? "";
    if (key === "") throw new Error("TOOLU_LIVE_JEV=1 needs TYPESAFE_API_KEY in the environment");
    using sb = createSandbox();
    selectJev(sb);
    const hooks = await createTooluHooks(binding(sb, [], key));
    try {
      const mandate = (await systemLines(hooks, "ses_a")).find((line) => line.includes(MANDATE));
      if (mandate === undefined) throw new Error("no Jev mandate");
      const env = { ...(await bashEnv(hooks, sb)), TYPESAFE_API_KEY: key };
      const question = "'Does the evidence describe a parser?' -s 'src/parser.ts exports parse()'";
      const res = await inShell(sb, `${calledCommand(mandate)} noul ${question}`, env);
      process.stdout.write(`live api.typesafe.ai: exit ${res.exitCode} ${res.stdout}`);
      expect(res.exitCode).toBe(0);
      expect(res.stdout).toMatch(/^\{"q":\{"type":"noul","noul":(0(\.\d+)?|1)\}\}\n$/);
    } finally {
      await hooks.dispose?.();
    }
  },
  120_000,
);
