/**
 * Live jira proof (#351, AC-9), reported apart from the hermetic suite.
 * `TOOLU_LIVE_OPENCODE=1` drives the pinned host: the model sees the
 * instruction and the skill, loading the skill reaches no Jira, and its bash
 * runs the published helper against the loopback fixture with the environment's
 * credentials, never the project `.env`'s.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
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

const PROMPT = "PROBE:jira.issue what is the status of ABC-1";
const INSTRUCTION = "jira (issue tracker)";
const SECRET = "dotenv-jira-secret";
const TOKEN = "env-jira-token";
const SELECTION = JSON.stringify({ version: 1, enabled: ["toolu", "jira"] });
const ISSUE = readFileSync(
  join(ROOT, "plugins/jira/hooks/src/__tests__/fixtures/issue.json"),
  "utf8",
);

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "the pinned host gives the model jira's instruction and skill, and its bash reads an issue",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    const fixture = await startHttpsFixture(["acme.atlassian.net", "media.example.net"]);
    const proxy = Object.entries(fixture.env)
      .map(([name, value]) => `${name}='${value}'`)
      .join(" ");
    // `env -u` keeps a developer's own Jira setup out; only the project .env could add one.
    const get = `env -u JIRA_EMAIL -u JIRA_API_TOKEN JIRA_CLI_CONFIG=/dev/null NETRC=/dev/null JIRA_BASE_URL=https://acme.atlassian.net JIRA_PAT=${TOKEN} ${proxy} "$TOOLU_BUN" --no-env-file "$TOOLU_CONFIG_DIR/jira/jira.sh" issue get ABC-1`;
    try {
      using session = openSession(cacheRoot, {
        config: () => ({ permission: { bash: "allow", skill: "allow" } }),
        files: {
          ...PROJECT_FILES,
          ".env": `JIRA_BASE_URL=https://media.example.net\nJIRA_PAT=${SECRET}\n`,
          ".opencode/toolu/plugins.json": SELECTION,
        },
        scripts: {
          "jira.issue": [
            { tool: "skill", args: { name: "jira-jira" } },
            { tool: "bash", args: { command: get, description: "jira issue get" } },
          ],
        },
      });
      installShim(session);
      session.env.TOOLU_BUN = process.execPath;
      session.env.TOOLU_REPO_ROOT = ROOT;
      fixture.plan([{ body: ISSUE }]);
      const hostRun = await runHost(host.bin, session, ["--print-logs", PROMPT]);
      expect(hostRun.exitCode).toBe(0);
      expect(diagnostics(hostRun.stderr, "toolu: ready")).toBe(1);
      const system = messagesText(session, "system");
      const captured = allRequestText(session);
      expect(system).toContain(INSTRUCTION);
      expect(system).toContain("--no-env-file");
      // Discovered, not merely named by the instruction: the host lists it as a skill.
      const skills = system.includes(String.raw`<name>jira-jira</name>`);
      expect(skills).toBe(true);
      expect(toolStates(hostRun)).toEqual([
        { tool: "skill", status: "completed", error: null },
        { tool: "bash", status: "completed", error: null },
      ]);
      expect(fixture.requests.map((req) => `${req.method} ${req.headers.host}${req.path}`)).toEqual(
        ["GET acme.atlassian.net/rest/api/3/issue/ABC-1"],
      );
      expect(fixture.requests[0]?.headers.authorization).toBe(`Bearer ${TOKEN}`);
      expect(captured).toContain("ABC-123");
      expect(captured).not.toContain(SECRET);
      process.stdout.write(
        `${JSON.stringify({
          host: host.version,
          instruction: system.includes(INSTRUCTION),
          skill: skills,
          tools: toolStates(hostRun).map((state) => `${state.tool}:${state.status}`),
          fixtureRequests: fixture.requests.map((req) => `${req.method} ${req.path}`),
          dotenvSent: fixture.requests.some((req) =>
            (req.headers.authorization ?? "").includes(SECRET),
          ),
        })}\n`,
      );
    } finally {
      await fixture.stop();
    }
  },
  600_000,
);
