/** Shared setup for the pinned-host pre-tool smoke scenarios. */
import { mkdirSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import type { toolStates } from "./host-run.ts";
import { openSession, PROBE_PLUGIN, type ProbeSession, type SessionOptions } from "./session.ts";
import type { ScenarioContext } from "./scenario.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const PACKAGE_DIR = join(ROOT, "tools/toolu-opencode");
export const ENV_BYTES = "SECRET=1\n";
export const SMOKE_RUN_TIMEOUT_MS = 300_000;

type Observed = Record<string, boolean | string>;
export type PretoolScenario = {
  id: string;
  run: (ctx: ScenarioContext) => Promise<{ pass: boolean; observed: Observed }>;
};

/** Keep pinned-host smoke scenarios serial so isolated profiles share the CLI cache safely. */
export async function runPretoolScenarios(
  ctx: ScenarioContext,
  remaining: readonly PretoolScenario[],
): Promise<number> {
  const [scenario, ...rest] = remaining;
  if (scenario === undefined) return 0;
  const result = await scenario.run(ctx);
  process.stdout.write(
    `${scenario.id} ${result.pass ? "pass" : "FAIL"} ${JSON.stringify(result.observed)}\n`,
  );
  return (result.pass ? 0 : 1) + (await runPretoolScenarios(ctx, rest));
}

export function session(ctx: ScenarioContext, options: SessionOptions): ProbeSession {
  const { files: extraFiles, config: extraConfig, ...rest } = options;
  const opened = openSession(ctx.cacheRoot, {
    ...rest,
    localPlugins: [PROBE_PLUGIN],
    files: {
      ".env": ENV_BYTES,
      ".opencode/toolu/plugins.json": JSON.stringify({ version: 1, enabled: ["toolu"] }),
      ...extraFiles,
    },
    config: (root) => ({
      permission: { bash: "allow", edit: "allow", task: "allow" },
      ...extraConfig?.(root),
    }),
  });
  opened.sb.write(".opencode/plugins/toolu.ts", 'export { default } from "@toolu/opencode";\n');
  mkdirSync(join(opened.sb.project, "node_modules/@toolu"), { recursive: true });
  symlinkSync(PACKAGE_DIR, join(opened.sb.project, "node_modules/@toolu/opencode"));
  opened.env.TOOLU_BUN = process.execPath;
  opened.env.TOOLU_REPO_ROOT = ROOT;
  return opened;
}

export function denied(
  states: ReturnType<typeof toolStates>,
  tool: string,
  reason: RegExp,
): boolean {
  return states.some(
    (state) => state.tool === tool && state.status === "error" && reason.test(state.error ?? ""),
  );
}
