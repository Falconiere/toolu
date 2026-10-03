/**
 * Shared by the hermetic and live delivery-workflow suites (#355): a git
 * project selecting delivery-flow with a local bare remote, Draft and Approved
 * spec and plan docs, and the ledger and verdict commands the generated
 * delivery skill tells the model to run.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { GENERATED, PASSING_TEST } from "./core-fixtures.ts";
import { gitProject } from "./workflow-fixtures.ts";

export const BRANCH = "feat/deliver";
export const SKILL_DIR = join(GENERATED, "skills", "delivery-flow-delivery-flow");
export const SPEC = "docs/spec.md";
export const PLAN = "docs/plan.md";
export const LEDGER = ".opencode/tmp/plan-ledger/feat_deliver.json";
export const REMEDY = 'skill({ name: "delivery-flow-delivery-flow" })';

type Status = "Draft" | "Approved";

/** The generated execution reference's `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/<entry>.js"` prefix. */
function generatedCommand(entry: "plan-ledger" | "verdict"): string {
  const text = readFileSync(join(SKILL_DIR, "references", "execution.md"), "utf8");
  const command = `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/${entry}.js"`;
  if (!text.includes(`\`${command} `)) throw new Error(`no ${entry} command in execution.md`);
  return command;
}

export const PLAN_LEDGER = generatedCommand("plan-ledger");
export const VERDICT = `${generatedCommand("verdict")} status`;

export const specDoc = (status: Status): string =>
  `# Math\n\n**Status:** ${status}\n\n- **AC-1:** two numbers add.\n`;

export const planDoc = (status: Status): string => `# Math plan

**Status:** ${status}
**Spec:** ${SPEC}

## Steps (machine-readable)

\`\`\`json
[
  { "id": "test", "title": "math test passes", "check": "bun test", "ac_refs": ["AC-1"] },
  { "id": "docs", "title": "readme documents math", "check": "grep -q math README.md" }
]
\`\`\`
`;

/**
 * `sb` (created with `git: true`) as a project selecting delivery-flow with
 * `planLedger.mode: block`, `BRANCH` changing a passing test, a README and
 * Approved docs one commit ahead of `main` on a bare remote. Returns the
 * remote's path.
 */
export function deliveryProject(sb: Sandbox): string {
  return gitProject(sb, {
    branch: BRANCH,
    selection: { version: 1, enabled: ["delivery-flow"] },
    gates: { planLedger: { mode: "block" } },
    files: {
      "math.test.ts": PASSING_TEST,
      "README.md": "# math\n\nAdds two numbers.\n",
      [SPEC]: specDoc("Approved"),
      [PLAN]: planDoc("Approved"),
    },
  });
}
