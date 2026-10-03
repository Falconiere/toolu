/**
 * Live context7 proof (#348, AC-6), reported apart from the hermetic suite.
 * `TOOLU_LIVE_OPENCODE=1` drives the pinned host: the model sees the
 * instruction and the skill, and its bash runs the instruction's command
 * against the loopback fixture without the project `.env` key.
 * `TOOLU_LIVE_CONTEXT7=1` makes one real context7.com search through the same
 * published command and `shell.env` environment.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
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
import { bashEnv, binding, inShell, systemLines } from "./jev-fixtures.ts";

const PROMPT = "PROBE:context7.docs how do I clean up a React effect";
const INSTRUCTION = "context7 (library docs)";
const SECRET = `${"ctx7sk_"}dotenv-secret`;
const SELECTION = JSON.stringify({ version: 1, enabled: ["toolu", "context7"] });
const LibrariesSchema = z.object({ results: z.array(z.object({ id: z.string() })) });
const LIBRARIES = { results: [{ id: "/facebook/react", title: "React", totalSnippets: 7 }] };

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "the pinned host gives the model context7's instruction and skill, and its bash runs the helper",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    const fixture = await startHttpsFixture(["context7.com"]);
    const proxy = Object.entries(fixture.env)
      .map(([name, value]) => `${name}='${value}'`)
      .join(" ");
    // `env -u` keeps a developer's own key out; only a project .env could add one.
    const search = `env -u CONTEXT7_API_KEY ${proxy} "$TOOLU_BUN" --no-env-file "$TOOLU_CONFIG_DIR/context7/search.sh" search react`;
    try {
      using session = openSession(cacheRoot, {
        config: () => ({ permission: { bash: "allow" } }),
        files: {
          ...PROJECT_FILES,
          ".env": `CONTEXT7_API_KEY=${SECRET}\n`,
          ".opencode/toolu/plugins.json": SELECTION,
        },
        scripts: {
          "context7.docs": [{ tool: "bash", args: { command: search, description: "context7" } }],
        },
      });
      installShim(session);
      session.env.TOOLU_BUN = process.execPath;
      session.env.TOOLU_REPO_ROOT = ROOT;
      fixture.plan([{ body: JSON.stringify(LIBRARIES) }]);
      const hostRun = await runHost(host.bin, session, ["--print-logs", PROMPT]);
      expect(hostRun.exitCode).toBe(0);
      expect(diagnostics(hostRun.stderr, "toolu: ready")).toBe(1);
      const system = messagesText(session, "system");
      const captured = allRequestText(session);
      expect(system).toContain(INSTRUCTION);
      expect(system).toContain("--no-env-file");
      expect(captured).toContain("context7-context7");
      expect(toolStates(hostRun)).toEqual([{ tool: "bash", status: "completed", error: null }]);
      expect(fixture.requests).toHaveLength(1);
      expect(fixture.requests[0]?.path).toBe("/api/v2/libs/search?libraryName=react&query=react");
      expect(fixture.requests[0]?.headers.authorization).toBeUndefined();
      expect(captured).toContain("/facebook/react");
      expect(captured).not.toContain(SECRET);
      process.stdout.write(
        `${JSON.stringify({
          host: host.version,
          instruction: system.includes(INSTRUCTION),
          skill: captured.includes("context7-context7"),
          search: captured.includes("/facebook/react"),
          fixtureRequests: fixture.requests.length,
          dotenvSent: fixture.requests.some((req) => req.headers.authorization !== undefined),
        })}\n`,
      );
    } finally {
      await fixture.stop();
    }
  },
  600_000,
);

test.skipIf(process.env.TOOLU_LIVE_CONTEXT7 !== "1")(
  "one real context7.com search runs through the OpenCode-published command",
  async () => {
    using sb = createSandbox();
    mkdirSync(join(sb.project, ".opencode/toolu"), { recursive: true });
    writeFileSync(join(sb.project, ".opencode/toolu/plugins.json"), SELECTION);
    const hooks = await createTooluHooks(binding(sb, [], ""));
    try {
      const line = (await systemLines(hooks, "ses_a")).find((text) => text.includes(INSTRUCTION));
      const command = line?.split("you MUST run ")[1]?.split(" FIRST")[0];
      if (command === undefined) throw new Error("no context7 instruction");
      const env = { ...(await bashEnv(hooks, sb)), CONTEXT7_API_KEY: undefined };
      const res = await inShell(sb, `${command} search react`, env);
      expect(res.exitCode).toBe(0);
      const body = LibrariesSchema.parse(JSON.parse(res.stdout));
      process.stdout.write(
        `live context7.com: exit ${res.exitCode}, ${body.results.length} results, first ${body.results[0]?.id}\n`,
      );
      expect(body.results.length).toBeGreaterThan(0);
    } finally {
      await hooks.dispose?.();
    }
  },
  120_000,
);
