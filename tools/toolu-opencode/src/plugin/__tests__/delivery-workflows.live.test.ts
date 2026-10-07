/**
 * Live delivery-workflow proof (#355, AC-8), reported apart from the hermetic
 * suite. `TOOLU_LIVE_OPENCODE=1` drives the pinned host with a scripted model
 * and no `external_directory` rule: it loads the delivery and brainstorm skills
 * through the native `skill` tool, reads one installed reference of each, sees
 * preflight refuse a Draft plan and accept the Approved one, verifies the
 * ledger, records the review and pushes.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { runHost, toolStates } from "../../../../../tooling/src/opencode-host/host-run.ts";
import {
  hostCacheDir,
  resolveHostBinary,
} from "../../../../../tooling/src/opencode-host/install.ts";
import { contractPaths } from "../../../../../tooling/src/opencode-host/results.ts";
import { allRequestText } from "../../../../../tooling/src/opencode-host/scenario.ts";
import {
  ROOT,
  diagnostics,
  installShim,
} from "../../../../../tooling/src/opencode-host/scenarios-entry.ts";
import { PinSchema, readJson } from "../../../../../tooling/src/opencode-host/schema.ts";
import { openSession } from "../../../../../tooling/src/opencode-host/session.ts";
import { GENERATED, writeStateCommand } from "./core-fixtures.ts";
import {
  BRANCH,
  PLAN,
  PLAN_LEDGER,
  REMEDY,
  SKILL_DIR,
  deliveryProject,
  planDoc,
  tooluBin,
} from "./delivery-fixtures.ts";

const PROMPT = "PROBE:delivery.workflows load, preflight, verify and push";
const DRAFT_PLAN = "docs/draft-plan.md";
const bash = (command: string) => ({ tool: "bash", args: { command, description: "deliver" } });
const read = (filePath: string) => ({ tool: "read", args: { filePath } });

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "the pinned host loads both skills and their references and delivers through the ledger",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    using session = openSession(cacheRoot, {
      config: () => ({ permission: { bash: "allow" } }),
      scripts: {
        "delivery.workflows": [
          { tool: "skill", args: { name: "delivery-flow-delivery-flow" } },
          read(join(SKILL_DIR, "references/spec.md")),
          { tool: "skill", args: { name: "brainstorm-brainstorm" } },
          read(join(GENERATED, "skills/brainstorm-brainstorm/references/design-questions.md")),
          bash(`${PLAN_LEDGER} preflight ${DRAFT_PLAN}`),
          bash(`${PLAN_LEDGER} preflight ${PLAN}`),
          bash(`${PLAN_LEDGER} run ${PLAN} --verify`),
          bash(writeStateCommand(0)),
          bash(`git push origin ${BRANCH}`),
        ],
      },
    });
    const remote = deliveryProject(session.sb);
    session.sb.write(DRAFT_PLAN, planDoc("Draft"));
    installShim(session);
    // The skill's `toolu ledger …` resolves to the TOOLU_IMPL seam's shim (#421).
    session.env.PATH = `${tooluBin(session.sb)}:${process.env.PATH ?? ""}`;
    session.env.TOOLU_BUN = process.execPath;
    session.env.TOOLU_REPO_ROOT = ROOT;

    const hostRun = await runHost(host.bin, session, ["--print-logs", PROMPT]);
    expect(hostRun.exitCode).toBe(0);
    expect(diagnostics(hostRun.stderr, "toolu: ready")).toBe(1);
    const states = toolStates(hostRun);
    expect(states.map(({ tool, status }) => `${tool}:${status}`)).toEqual([
      "skill:completed",
      "read:completed",
      "skill:completed",
      "read:completed",
      "bash:completed",
      "bash:completed",
      "bash:completed",
      "bash:completed",
      "bash:completed",
    ]);
    const captured = allRequestText(session);
    expect(captured).toContain("Private delivery-flow spec phase.");
    expect(captured).toContain("# Design question bank");
    const refused = `preflight: plan not approved (Status: Draft) — load ${REMEDY} (plan review phase)`;
    expect(captured).toContain(JSON.stringify(refused).slice(1, -1));
    expect(captured).toContain("2/2 fresh-green");
    const pushed = session.sb.git("--git-dir", remote, "rev-parse", `refs/heads/${BRANCH}`).trim();
    expect(pushed).toBe(session.sb.git("rev-parse", "HEAD").trim());
    process.stdout.write(
      `${JSON.stringify({ host: host.version, tools: states.map((s) => `${s.tool}:${s.status}`), pushed })}\n`,
    );
  },
  900_000,
);
