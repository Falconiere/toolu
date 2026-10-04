/**
 * Regression controls (#362, AC-1): proof that the acceptance would notice the
 * regressions the epic was opened for. Each control stages a copy of
 * `@toolu/opencode`, breaks it in one way, and runs the check that must catch
 * that break against the copy on the pinned host. A control passes only when
 * its check fails. Every edit asserts that it matched, so a source change can
 * never turn a control into a silent no-op.
 */
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { stageOpencode } from "../npm-pack.ts";
import { ROOT } from "../opencode-host/scenarios-entry.ts";
import { ContractError } from "../opencode-host/schema.ts";
import {
  inSequence,
  type AcceptanceCheck,
  type AcceptanceContext,
  type CheckOutcome,
} from "./checks.ts";
import type { ControlResult } from "./report.ts";

type Control = { id: string; regression: string; check: string; apply: (stage: string) => void };

/** Replace `from` in `file`, which must hold exactly `count` occurrences of it. */
export function replaceExactly(file: string, from: string, to: string, count = 1): void {
  if (!existsSync(file)) throw new ContractError(`control edit: ${file} does not exist`);
  const text = readFileSync(file, "utf8");
  const found = text.split(from).length - 1;
  if (found !== count) {
    throw new ContractError(
      `control edit: ${file} has ${found} of ${JSON.stringify(from)}, expected ${count}`,
    );
  }
  writeFileSync(file, text.replaceAll(from, to));
}

/**
 * The retired V2 entry (`@opencode/plugin@2`): `Plugin.define` with `setup` and
 * a `permission.hook("evaluate")` that would deny the `.env` write under that
 * contract. The pinned host never calls it.
 */
const V2_ENTRY = `const Plugin = { define: (plugin) => plugin };
export default Plugin.define({
  id: "toolu",
  async setup(ctx) {
    await ctx.permission.hook("evaluate", async (event) => {
      if (event.resources.some((path) => path.endsWith(".env"))) event.effect = "deny";
    });
  },
});
`;

function replaceEntry(stage: string): void {
  const entry = join(stage, "src/plugin/toolu.ts");
  if (!existsSync(entry) || !readFileSync(entry, "utf8").includes("export default")) {
    throw new ContractError(`control edit: ${entry} has no default export to replace`);
  }
  writeFileSync(entry, V2_ENTRY);
}

/** `brainstorm-brainstorm` becomes `brainstorm--brainstorm`, the double-hyphen id the old generator produced. */
function invalidSkillName(stage: string): void {
  const generated = join(stage, "generated");
  replaceExactly(
    join(generated, "opencode.toolu.json"),
    "brainstorm-brainstorm",
    "brainstorm--brainstorm",
    2,
  );
  const skill = join(generated, "skills/brainstorm-brainstorm");
  replaceExactly(
    join(skill, "SKILL.md"),
    'name: "brainstorm-brainstorm"',
    'name: "brainstorm--brainstorm"',
  );
  renameSync(skill, join(generated, "skills/brainstorm--brainstorm"));
}

function dropHook(key: string): (stage: string) => void {
  return (stage) => replaceExactly(join(stage, "src/plugin/hooks.ts"), key, "");
}

export const CONTROLS: readonly Control[] = [
  {
    id: "control.v2-entry",
    regression: 'the entry is the V2 Plugin.define/permission.hook("evaluate") shape',
    check: "entry.local-shim",
    apply: replaceEntry,
  },
  {
    id: "control.invalid-skill-name",
    regression: "a generated skill name has a double hyphen",
    check: "surface.discovered-names",
    apply: invalidSkillName,
  },
  {
    id: "control.missing-context",
    regression: "the experimental.chat.system.transform hook is not wired",
    check: "live.context-delivery",
    apply: dropHook('"experimental.chat.system.transform": context.system,'),
  },
  {
    id: "control.missing-post-tool",
    regression: "the tool.execute.after hook is not wired",
    check: "posttool.edit",
    apply: dropHook('"tool.execute.after": enforcement.after,'),
  },
];

/** Link `target` at `path`; a missing target is an error, never a dangling link. */
function linkExisting(target: string, path: string): void {
  if (!existsSync(target))
    throw new ContractError(`control stage: ${target} does not exist (run bun install)`);
  symlinkSync(target, path);
}

/** A staged package whose imports resolve from the checkout's installed dependencies. */
export function stageControl(work: string): string {
  const stage = stageOpencode(work);
  linkExisting(join(ROOT, "tools/toolu-opencode/node_modules"), join(stage, "node_modules"));
  linkExisting(join(ROOT, "node_modules"), join(work, "repo/node_modules"));
  return stage;
}

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * Run `check` with the shim pointed at `stage`; a throw means the check did not
 * pass. The override is process-wide, so a second control while one is running
 * is refused instead of silently testing the wrong package.
 */
async function againstStage(
  check: AcceptanceCheck,
  stage: string,
  ctx: AcceptanceContext,
): Promise<CheckOutcome> {
  const active = process.env.TOOLU_ACCEPTANCE_PACKAGE;
  if (active !== undefined && active !== "")
    throw new ContractError(
      `a control is already running against ${active}; controls run one at a time`,
    );
  process.env.TOOLU_ACCEPTANCE_PACKAGE = stage;
  try {
    return await check.run(ctx);
  } catch (err) {
    return { pass: false, observed: { error: errorText(err) } };
  } finally {
    delete process.env.TOOLU_ACCEPTANCE_PACKAGE;
  }
}

/** The controls a run selects: all of them in a complete run, else those named in `only`. */
export function selectControls(only: readonly string[]): Control[] {
  return only.length === 0
    ? [...CONTROLS]
    : CONTROLS.filter((control) => only.includes(control.id));
}

/** Each control in order, against its check from the full registry. */
export function runControls(
  controls: readonly Control[],
  registry: readonly AcceptanceCheck[],
  ctx: AcceptanceContext,
): Promise<ControlResult[]> {
  return inSequence(controls, async (control) => {
    const check = registry.find((item) => item.id === control.check);
    if (check === undefined)
      throw new ContractError(`${control.id} names an unknown check: ${control.check}`);
    const result = await runControl(control, check, ctx);
    const verdict = result.detected ? "detected" : "MISSED";
    process.stdout.write(
      `${control.id} ${verdict} ${result.error ?? JSON.stringify(result.observed)}\n`,
    );
    return result;
  });
}

/**
 * One control. Its check must first pass against the unbroken stage, which
 * proves the stage itself works; then the same stage gets the one edit and
 * the check must fail. Only that pair is detection: a stage that fails on its
 * own, a failed edit, or a check that still passes is a missed control.
 */
export async function runControl(
  control: Control,
  check: AcceptanceCheck,
  ctx: AcceptanceContext,
): Promise<ControlResult> {
  const base = { id: control.id, regression: control.regression, check: control.check };
  const work = mkdtempSync(join(tmpdir(), "toolu-acceptance-control-"));
  try {
    const stage = stageControl(work);
    const pristine = await againstStage(check, stage, ctx);
    if (!pristine.pass) {
      const error = `${control.check} fails on the unbroken stage, so a failure proves nothing`;
      return { ...base, detected: false, observed: { pristine: pristine.observed }, error };
    }
    control.apply(stage);
    const broken = await againstStage(check, stage, ctx);
    return {
      ...base,
      detected: !broken.pass,
      observed: { pristine: "pass", broken: broken.observed },
    };
  } catch (err) {
    return { ...base, detected: false, observed: {}, error: errorText(err) };
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}
