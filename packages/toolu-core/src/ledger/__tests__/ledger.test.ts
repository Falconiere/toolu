/**
 * AC-9 (#256): the `@toolu/core/ledger` package export resolves through
 * package.json. Through it, the self-test parses the same two-step fixture
 * as `plan-ledger.sh --self-test`, and the public surface is present.
 */
import { expect, test } from "bun:test";
import { readFileSync, writeFileSync } from "node:fs";
import * as ledger from "@toolu/core/ledger";
import { diffSha } from "@toolu/core/state";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { childEnv } from "@toolu/conformance/harness/spawn";

test("@toolu/core/ledger runs the self-test through the export map", async () => {
  expect(await ledger.ledgerMain(["--self-test"])).toEqual({
    exitCode: 0,
    stdout: "plan-ledger --self-test: ok\n",
    stderr: "",
  });
});

test("@toolu/core/ledger exposes the ledger, verdict and waiver layer", () => {
  for (const name of [
    "ledgerMain",
    "ledgerRun",
    "ledgerStatus",
    "ledgerPreflight",
    "parseSteps",
    "docField",
    "parseAcs",
    "checkAcRefs",
    "readLedger",
    "writeLedger",
    "ledgerPath",
    "recompute",
    "summaryLine",
    "healOrphans",
    "verdictMain",
    "verdictReport",
    "renderVerdictStatus",
    "pushWaiverPend",
    "pushWaiverPromote",
    "pushWaiverMatches",
  ] as const) {
    expect(typeof ledger[name]).toBe("function");
  }
  expect(ledger.PUSH_WAIVER_VERSION).toBe(1);
  expect(ledger.ACCEPTED_REVIEWERS).toContain("toolu-review:review");
});

const FIXED = new Date("2031-02-03T04:05:06Z");
const STAMP = "2031-02-03T04:05:06Z";

function repo() {
  const sb = createSandbox({ git: true, files: { "base.txt": "base\n" } });
  sb.git("checkout", "-q", "-b", "feat/x");
  sb.write("a.ts", "x\n");
  sb.git("add", "a.ts");
  sb.git("commit", "-qm", "a");
  sb.write(
    "plan.md",
    '## Steps (machine-readable)\n\n```json\n[{"id":"s1","title":"t","check":"echo out >&2; true"}]\n```\n',
  );
  return sb;
}

const env = (sb: { home: string }) =>
  childEnv({
    HOME: sb.home,
    PUSH_REVIEW_BASE: "main",
    TOOLU_HOST_OVERRIDE: "claude",
    TOOLU_CONFIG_DIR: sb.home,
  });

/** `preflight` on a plan and its declared spec, both stamped as given, under `host`. */
async function preflight(host: string, planStatus: string, specStatus: string) {
  using sb = repo();
  sb.write("spec.md", `# S\n\n**Date:** 2031-02-03   **Status:** ${specStatus}\n`);
  sb.write("p.md", `# P\n\n**Status:** ${planStatus}   **Spec:** spec.md\n`);
  return ledger.ledgerMain(["preflight", "p.md"], {
    cwd: sb.project,
    env: { ...env(sb), TOOLU_HOST_OVERRIDE: host },
  });
}

const OPENCODE_REMEDY = 'load skill({ name: "delivery-flow-delivery-flow" })';

test.concurrent("preflight on OpenCode names the generated delivery-flow skill", async () => {
  const plan = await preflight("opencode", "Draft", "Approved");
  expect(plan.exitCode).toBe(1);
  expect(plan.stderr).toBe(
    `preflight: plan not approved (Status: Draft) — ${OPENCODE_REMEDY} (plan review phase)\n`,
  );
  const spec = await preflight("opencode", "Approved", "Draft");
  expect(spec.exitCode).toBe(1);
  expect(spec.stderr).toBe(
    `preflight: spec spec.md not approved (Status: Draft) — ${OPENCODE_REMEDY} (spec review phase)\n`,
  );
  expect(await preflight("opencode", "Approved", "Approved")).toMatchObject({
    exitCode: 0,
    stderr: "",
  });
});

test.concurrent.each(["claude", "codex"])(
  "preflight on %s keeps its slash command",
  async (host) => {
    const plan = await preflight(host, "Draft", "Approved");
    expect(plan.exitCode).toBe(1);
    expect(plan.stderr).toBe(
      "preflight: plan not approved (Status: Draft) — run /delivery-flow:delivery-flow (plan review phase)\n",
    );
    const spec = await preflight(host, "Approved", "Draft");
    expect(spec.stderr).toContain("run /delivery-flow:delivery-flow (spec review phase)");
  },
);

test("onStderr sees exactly the returned stderr lines, in order, as they are emitted", async () => {
  using sb = repo();
  const seen: string[] = [];
  const res = await ledger.ledgerMain(["run", sb.path("plan.md")], {
    cwd: sb.project,
    env: env(sb),
    onStderr: (line) => seen.push(line),
  });
  expect(res.exitCode).toBe(0);
  expect(seen.length).toBeGreaterThan(0);
  expect(seen.map((line) => `${line}\n`).join("")).toBe(res.stderr);
});

test("now is the one clock for ledger, orphan cutoff, verdict and waiver stamps", async () => {
  using sb = repo();
  const o = { cwd: sb.project, env: env(sb), now: () => FIXED };
  await ledger.ledgerMain(["run", sb.path("plan.md")], o);
  const file = sb.path(".claude/tmp/plan-ledger/feat_x.json");
  const written = JSON.parse(readFileSync(file, "utf8")) as {
    updated_at: string;
    steps: { last_run: string }[];
  };
  expect(written.updated_at).toBe(STAMP);
  expect(written.steps[0]?.last_run).toBe(STAMP);

  // Started 299 s before the fixed clock: still live under the 300 s default; 301 s: orphaned.
  for (const [age, status] of [
    [299, "running"],
    [301, "pending"],
  ] as const) {
    const started = new Date(FIXED.getTime() - age * 1000).toISOString().slice(0, 19) + "Z";
    writeFileSync(
      file,
      JSON.stringify({
        ...written,
        steps: [{ ...written.steps[0], status: "running", started_at: started }],
      }),
    );
    ledger.ledgerStatus(o);
    expect(
      (JSON.parse(readFileSync(file, "utf8")) as { steps: { status: string }[] }).steps[0]?.status,
    ).toBe(status);
  }

  const report = JSON.parse(ledger.verdictMain(["json"], o).stdout) as { generated_at: string };
  expect(report.generated_at).toBe(STAMP);
  const sha = diffSha(sb.project, "main") ?? "";
  expect(sha).toMatch(/^[0-9a-f]{40,64}$/);
  expect(ledger.pushWaiverPend(sb.project, "feat_x", sha, "main", "no-state", o)).toBe(true);
  expect(
    JSON.parse(readFileSync(ledger.pushWaiverPendingPath(sb.project, "feat_x", o), "utf8")),
  ).toMatchObject({ asked_at: STAMP });
  expect(ledger.pushWaiverPromote(sb.project, "feat_x", sha, o)).toBe(true);
  expect(ledger.pushWaiverMatches(sb.project, "feat_x", sha, o)).toBe(true);
  expect(
    JSON.parse(readFileSync(ledger.pushWaiverPath(sb.project, "feat_x", o), "utf8")),
  ).toMatchObject({ waived_at: STAMP });
});
