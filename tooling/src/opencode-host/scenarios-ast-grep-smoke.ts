/**
 * Pinned-host proof for ast-grep on OpenCode (#347): the native skill loads,
 * its search example runs on a real project, search-nudge reaches the model
 * with the right results, and byte-savings records and reports the session.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import { session, SMOKE_RUN_TIMEOUT_MS, type PretoolScenario } from "./pretool-shared.ts";
import type { ProbeSession } from "./session.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";
import { prepareSdk } from "./scenarios-posttool-smoke.ts";
import { ContractError } from "./schema.ts";

const SKILL = "ast-grep-ast-grep";
const ADVISORY = "[toolu advisory]";
const REPORT = "ast-grep byte savings this session:";
const SEARCH = "ast-grep run -p 'export function $N($$$) { $$$ }' -l typescript src";
const LEDGER_DIR = ".opencode/toolu/state/toolu/byte-savings";
/** An earlier session's ledger, older and sorted first: the report must skip it. */
const OLD_LEDGER = "aaa-earlier-session.jsonl";
const REPORT_CLI =
  'bun "$TOOLU_PLUGIN_ROOT_AST_GREP/hooks/dist/byte-savings-report.js" "$(ls -t "$TOOLU_CONFIG_DIR"/toolu/byte-savings/*.jsonl | head -n 1)"';

function bash(command: string) {
  return { tool: "bash", args: { command, description: "ast-grep smoke" } };
}

/** The tool messages the model got, in call order. */
const STEPS = [
  { tool: "skill", args: { name: SKILL } },
  { tool: "grep", args: { pattern: "export function greet" } },
  { tool: "grep", args: { pattern: "TODO", include: "*.md" } },
  bash("rg -n 'function greet' src"),
  bash("rg -n TODO notes.md"),
  bash("git log --oneline | grep initial"),
  bash(SEARCH),
  { tool: "read", args: { filePath: "src/app.ts" } },
  bash(REPORT_CLI),
];

function count(text: string, needle: string): number {
  return text.split(needle).length - 1;
}

function ledgerKinds(project: string): string[] {
  const dir = join(project, LEDGER_DIR);
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((file) => file !== OLD_LEDGER)
    .flatMap((file) =>
      readFileSync(join(dir, file), "utf8")
        .trim()
        .split("\n")
        .map((line) => line.match(/"kind":"([^"]+)"/u)?.[1] ?? "?"),
    );
}

/** Each claim the session must prove, from its tool messages, tool states and project. */
function observe(
  m: readonly string[],
  states: ReturnType<typeof toolStates>,
  s: ProbeSession,
): Record<string, boolean> {
  const at = (i: number): string => m[i] ?? "";
  return {
    allCompleted:
      states.length === STEPS.length && states.every((state) => state.status === "completed"),
    skillLoaded: at(0).includes("ast-grep run -p") && !at(0).includes("detect.sh"),
    grepStructuralNudged:
      count(at(1), ADVISORY) === 1 && at(1).includes("STOP: Structural code pattern detected"),
    grepNonCodeQuiet: !at(2).includes(ADVISORY),
    rgStructuralNudged:
      count(at(3), ADVISORY) === 1 && at(3).includes("STOP: grep/rg for structural code search"),
    rgLiteralGeneric: count(at(4), ADVISORY) === 1 && at(4).includes("grep/rg in Bash detected"),
    pipedQuiet: !at(5).includes(ADVISORY),
    exampleFoundGreet: at(6).includes("export function greet"),
    reportOnAstGrep: count(at(6), REPORT) === 1 && at(6).includes("TOTAL returned:"),
    readQuiet: !at(7).includes(REPORT) && !at(7).includes(ADVISORY),
    reportCliRan:
      at(8).includes("ast-grep: returned=") &&
      at(8).includes("read: returned=") &&
      !at(8).includes("glob: returned="),
    earlierLedgerKept: s.exists(`${LEDGER_DIR}/${OLD_LEDGER}`),
    ledger: ledgerKinds(s.sb.project).toSorted().join(",") === "ast-grep,grep,grep,read",
  };
}

async function astGrepSession(ctx: ScenarioContext) {
  if (Bun.which("ast-grep") === null) {
    throw new ContractError("ast-grep is not on PATH; the skill example cannot run");
  }
  using s = session(ctx, {
    files: {
      ".opencode/toolu/plugins.json": JSON.stringify({
        version: 1,
        enabled: ["toolu", "ast-grep"],
      }),
      "src/app.ts": "export function greet(name: string) {\n  return name;\n}\n",
      "notes.md": "TODO: write notes\n",
      [`${LEDGER_DIR}/${OLD_LEDGER}`]: '{"kind":"glob","returned":1,"full":0}\n',
    },
    scripts: (project) => ({
      "ast-grep.session": STEPS.map((step) =>
        step.tool === "read"
          ? { tool: "read", args: { filePath: join(project, "src/app.ts") } }
          : step,
      ),
    }),
  });
  await prepareSdk(s, ctx.cacheRoot);
  const hostRun = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:ast-grep.session"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(hostRun);
  const m = finalMessages(s, "tool");
  const observed = observe(m, states, s);
  const pass = Object.values(observed).every(Boolean);
  return {
    pass,
    observed: pass
      ? observed
      : {
          ...observed,
          diagnostic: JSON.stringify({ m, states, ledger: ledgerKinds(s.sb.project) }).slice(
            0,
            7000,
          ),
        },
  };
}

export const AST_GREP_SCENARIOS: PretoolScenario[] = [
  { id: "ast-grep.session", run: astGrepSession },
];
