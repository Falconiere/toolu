/**
 * delivery-flow on OpenCode (#355): the generated skill's ledger and verdict
 * commands in the agent's bash refuse a Draft spec or plan, record steps in
 * the OpenCode ledger, and hold a push at the real plan-ledger gate until the
 * branch is verified and reviewed, all through `createTooluHooks`.
 */
import { expect, test } from "bun:test";
import type { Config } from "@opencode-ai/plugin";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { isPlainRecord } from "../../surfaces/merge.ts";
import { GENERATED, NATIVE_BIN, writeStateCommand } from "./core-fixtures.ts";
import {
  BRANCH,
  LEDGER,
  PLAN,
  PLAN_LEDGER,
  REMEDY,
  SKILL_DIR,
  SPEC,
  VERDICT,
  deliveryProject,
  planDoc,
  specDoc,
  tooluBin,
} from "./delivery-fixtures.ts";
import { binding, hook } from "./jev-fixtures.ts";
import { bash, refusal, remoteHead, withHooks } from "./workflow-fixtures.ts";

test.concurrent("selecting delivery-flow registers the delivery and brainstorm skills", async () => {
  using sb = createSandbox({ git: true });
  deliveryProject(sb);
  await withHooks(binding(sb, [], ""), async (hooks) => {
    const config: Config = {};
    await hook(hooks, "config")(config);
    const skills: unknown = Reflect.get(config, "skills");
    const paths = isPlainRecord(skills) && Array.isArray(skills.paths) ? skills.paths : [];
    expect(paths).toContain(SKILL_DIR);
    expect(paths).toContain(`${GENERATED}/skills/brainstorm-brainstorm`);
  });
});

test.concurrent("preflight refuses a Draft spec or plan by the generated skill's id", async () => {
  using sb = createSandbox({ git: true });
  deliveryProject(sb);
  const preflight = `${PLAN_LEDGER} preflight ${PLAN}`;
  const bin = tooluBin(sb);
  await withHooks(binding(sb, [], ""), async (hooks) => {
    sb.write(SPEC, specDoc("Draft"));
    const draftSpec = await bash(hooks, sb, preflight, bin);
    expect(draftSpec.exitCode).toBe(1);
    expect(draftSpec.stderr).toContain(
      `preflight: spec ${SPEC} not approved (Status: Draft) — load ${REMEDY} (spec review phase)`,
    );

    sb.write(PLAN, planDoc("Draft"));
    const draftPlan = await bash(hooks, sb, preflight, bin);
    expect(draftPlan.exitCode).toBe(1);
    expect(draftPlan.stderr).toContain(
      `preflight: plan not approved (Status: Draft) — load ${REMEDY} (plan review phase)`,
    );

    sb.write(SPEC, specDoc("Approved"));
    sb.write(PLAN, planDoc("Approved"));
    expect(await bash(hooks, sb, preflight, bin)).toMatchObject({ exitCode: 0, stderr: "" });
  });
});

test.concurrent("a push waits for every step, the verify stamp and the review", async () => {
  using sb = createSandbox({ git: true });
  const remote = deliveryProject(sb);
  const push = `git push origin ${BRANCH}`;
  const bin = tooluBin(sb);
  await withHooks(binding(sb, [], ""), async (hooks) => {
    const step = await bash(hooks, sb, `${PLAN_LEDGER} run ${PLAN} --step test`, bin);
    expect(step.stderr).toContain("plan-ledger: [1/2] test: green");
    expect(step.stdout).toContain("1/2 fresh-green, next=docs");
    const ledger: unknown = JSON.parse(sb.read(LEDGER));
    expect(ledger).toMatchObject({ version: 1, plan_doc: PLAN });

    const blocked = await bash(hooks, sb, VERDICT, bin);
    expect(blocked.exitCode).toBe(1);
    expect(blocked.stdout).toContain("overall: blocked");
    expect(await refusal(hooks, push)).toContain(
      "plan-ledger: push blocked — steps not fresh-green:\ndocs: pending",
    );
    expect(remoteHead(sb, remote, BRANCH)).toBe("");

    expect((await bash(hooks, sb, `${PLAN_LEDGER} run ${PLAN} --verify`, bin)).exitCode).toBe(0);
    expect((await bash(hooks, sb, writeStateCommand(0), NATIVE_BIN)).exitCode).toBe(0);
    const ready = await bash(hooks, sb, VERDICT, bin);
    expect(ready.stdout).toContain("overall: ready");
    expect(ready.exitCode).toBe(0);

    expect(await refusal(hooks, push)).toBe("allowed");
    expect((await bash(hooks, sb, push)).exitCode).toBe(0);
    expect(remoteHead(sb, remote, BRANCH)).toBe(sb.git("rev-parse", "HEAD").trim());
  });
});
