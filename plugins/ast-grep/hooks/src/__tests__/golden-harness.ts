/**
 * Runs one ast-grep golden case (#268) the way a host runs it: a real git
 * sandbox, ast-grep's modules registered into the host's registry, and the
 * call dispatched by toolu's committed pre-tools or post-tools bundle behind
 * its hooks.json launcher. The report cases run the report CLI directly.
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import { runPostBundle } from "@toolu/conformance/harness/posttool";
import { pretoolEnv, runBundle, type PretoolHost } from "@toolu/conformance/harness/pretool";
import {
  PRETOOL_CORPUS,
  prepare,
  pretoolStdin,
  type PretoolCase,
} from "@toolu/conformance/harness/pretool-corpus";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import type { NudgeCase, ReportCase, SavingsCase } from "./cases-types.ts";
import {
  PLUGIN_ROOT,
  baseSandbox,
  normalise,
  pathWithoutAstGrep,
  registerAstGrep,
  type Registration,
} from "./golden-sandbox.ts";

export type Captured = { stdout: string; stderr: string; exitCode: number };
export type SavingsCaptured = Captured & { ledgers: Record<string, string> };

export function caseKey(name: string, host: PretoolHost): string {
  return `${name} [${host}]`;
}

function captured(res: RunResult, sb: Sandbox): Captured {
  return {
    stdout: normalise(res.stdout, sb),
    stderr: normalise(res.stderr, sb),
    exitCode: res.exitCode,
  };
}

export async function runNudge(
  c: NudgeCase,
  host: PretoolHost,
  reg: Registration,
): Promise<Captured> {
  using sb = createSandbox({ git: true });
  baseSandbox(sb);
  if (c.state === "opt-out") sb.writeConfig(host, "project", { skills: { "ast-grep": false } });
  const extra = c.state === "missing" ? { PATH: pathWithoutAstGrep(sb) } : {};
  await registerAstGrep(sb, host, reg);
  const fixture = {
    kind: "tool",
    event: "PreToolUse",
    toolName: c.toolName,
    toolInput: c.toolInput,
  } as const;
  const stdin = JSON.stringify(toStdin(host, fixture, { cwd: sb.project }));
  return captured(
    await runBundle({ cwd: sb.project, env: pretoolEnv(sb, host, extra), stdin }),
    sb,
  );
}

/** Every file under a `byte-savings` directory anywhere in the sandbox, keyed from its root. */
function ledgers(sb: Sandbox): Record<string, string> {
  const out: Record<string, string> = {};
  const walk = (dir: string): void => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (name === ".git" || !statSync(path).isDirectory()) continue;
      if (name !== "byte-savings") {
        walk(path);
        continue;
      }
      for (const file of readdirSync(path).toSorted()) {
        out[relative(sb.root, join(path, file))] = readFileSync(join(path, file), "utf8");
      }
    }
  };
  walk(sb.root);
  return out;
}

export async function runSavings(
  c: SavingsCase,
  host: PretoolHost,
  reg: Registration,
): Promise<SavingsCaptured> {
  using sb = createSandbox({ git: true });
  baseSandbox(sb);
  c.setup?.(sb);
  const extra = c.env?.(sb) ?? {};
  await registerAstGrep(sb, host, reg, extra);
  const stdin = JSON.stringify({
    cwd: sb.project,
    hook_event_name: "PostToolUse",
    ...c.payload(sb),
  });
  const res = await runPostBundle(sb, { cwd: sb.project, env: pretoolEnv(sb, host, extra), stdin });
  return { ...captured(res, sb), ledgers: ledgers(sb) };
}

export async function runReport(c: ReportCase, reg: Registration): Promise<Captured> {
  using sb = createSandbox();
  const ledger = sb.path("ledger.jsonl");
  if (c.ledger !== undefined) writeFileSync(ledger, c.ledger);
  const argv =
    reg.kind === "bash"
      ? ["bash", join(reg.pluginRoot, "scripts/byte-savings-report.sh")]
      : entryArgv("ast-grep", "byte-savings-report", PLUGIN_ROOT);
  const res = await run(c.noArgument === true ? argv : [...argv, ledger], { cwd: sb.root });
  return captured(res, sb);
}

/** The #258 PreToolUse corpus fixtures search-nudge decides. */
export const CORPUS: readonly PretoolCase[] = PRETOOL_CORPUS.filter((f) =>
  f.name.startsWith("ast-grep registry:"),
);

export async function runCorpus(
  f: PretoolCase,
  host: PretoolHost,
  reg: Registration,
): Promise<Captured> {
  using sb = createSandbox({ git: true });
  const extra = await prepare(sb, host, f);
  await registerAstGrep(sb, host, reg);
  const stdin = pretoolStdin(sb, host, f);
  return captured(
    await runBundle({ cwd: sb.project, env: pretoolEnv(sb, host, extra), stdin }),
    sb,
  );
}

export function hostsOf(c: { hosts?: readonly PretoolHost[] }): readonly PretoolHost[] {
  return c.hosts ?? ["claude"];
}
