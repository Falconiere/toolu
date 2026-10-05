/**
 * Required-aggregate rules (#458). The aggregate fails when `changes` did not
 * succeed or left a group output that is not `true`/`false`, when a needed job ended in anything but success or skipped, when a
 * job was skipped although its group was on, and when `needs` differs from the
 * workflow's gated jobs. It passes only when every gated job either succeeded
 * or was skipped with its group off.
 */
import { z } from "zod";
import type { CiWorkflow } from "./config.ts";

/** GitHub's `toJSON(needs)`: job id → result and outputs. */
export const NeedsSchema = z.record(
  z.string(),
  z.looseObject({
    result: z.string(),
    outputs: z.record(z.string(), z.string()).optional(),
  }),
);
type Needs = z.infer<typeof NeedsSchema>;

type Verdict = { ok: boolean; lines: string[] };

const CHANGES = "changes";

function gatedLine(
  id: string,
  group: string,
  needs: Needs,
  outputs: Record<string, string>,
): string | null {
  const job = needs[id];
  if (job === undefined) return `${id}: missing from needs`;
  const on = outputs[group] === "true";
  const label = `${id} (${group} ${on ? "on" : "off"}): ${job.result}`;
  if (job.result === "success") return null;
  if (job.result === "skipped") return on ? `${label}, but its group is on` : null;
  return label;
}

/** Judge `needs` against `workflow`'s gated jobs. */
export function judge(workflow: CiWorkflow, needs: Needs): Verdict {
  const changes = needs[CHANGES];
  if (changes === undefined) return { ok: false, lines: [`${CHANGES}: missing from needs`] };
  if (changes.result !== "success") {
    return { ok: false, lines: [`${CHANGES}: ${changes.result}; no group decision to trust`] };
  }
  const outputs = changes.outputs ?? {};
  const groups = [...new Set(Object.values(workflow.jobs))];
  const malformed = groups.filter(
    (group) => outputs[group] !== "true" && outputs[group] !== "false",
  );
  if (malformed.length > 0) {
    const lines = malformed.map(
      (group) => `${CHANGES}: output ${group} is "${outputs[group] ?? ""}", not true or false`,
    );
    return { ok: false, lines };
  }
  const failures: string[] = [];
  for (const id of Object.keys(needs)) {
    if (id !== CHANGES && workflow.jobs[id] === undefined) {
      failures.push(`${id}: needed but not mapped to a group`);
    }
  }
  for (const [id, group] of Object.entries(workflow.jobs)) {
    const line = gatedLine(id, group, needs, outputs);
    if (line !== null) failures.push(line);
  }
  if (failures.length > 0) return { ok: false, lines: failures };
  const summary = Object.entries(workflow.jobs).map(
    ([id, group]) =>
      `${id} (${group} ${outputs[group] === "true" ? "on" : "off"}): ${needs[id]?.result ?? "missing"}`,
  );
  return { ok: true, lines: summary };
}
