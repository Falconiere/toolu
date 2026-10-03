/** What the language-quality pinned-host smokes share: project files, gate reads, the disabled case. */
import { join } from "node:path";
import { z } from "zod";
import { runHost, toolStates } from "./host-run.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";
import { session, SMOKE_RUN_TIMEOUT_MS } from "./pretool-shared.ts";
import { prepareSdk, verdict } from "./scenarios-posttool-smoke.ts";

const GATE = ".opencode/tmp/quality-gate-status.json";
const Gate = z.object({
  status: z.string(),
  entries: z
    .record(z.string(), z.object({ source: z.string(), violations: z.string() }))
    .optional(),
});
const CONFIG = JSON.stringify({
  version: 1,
  gates: {
    qualityGate: { mode: "block" },
    commitGate: { mode: "off" },
    pushReview: { mode: "off" },
  },
});

type Session = ReturnType<typeof session>;

/** A project selecting `toolu` plus `plugins`, with the quality gate blocking and `files` added. */
export function qualityProject(
  plugins: readonly string[],
  files: Record<string, string>,
): Record<string, string> {
  return {
    ".opencode/toolu/plugins.json": JSON.stringify({ version: 1, enabled: ["toolu", ...plugins] }),
    ".opencode/toolu.config.json": CONFIG,
    ...files,
  };
}

/** The project's parsed quality gate, or null when absent or malformed. */
export function qualityGate(s: Session): z.infer<typeof Gate> | null {
  if (!s.exists(GATE)) return null;
  const parsed = Gate.safeParse(JSON.parse(s.sb.read(GATE)));
  return parsed.success ? parsed.data : null;
}

type DisabledCase = {
  readonly id: string;
  readonly files: Record<string, string>;
  readonly file: string;
  readonly content: string;
  /** Runs after the project is written and before the host starts. */
  readonly prepare?: (s: Session) => void;
};

/** With only `toolu` selected, an invalid write lands but no quality module reports or gates it. */
export async function disabledQuality(ctx: ScenarioContext, c: DisabledCase) {
  using s = session(ctx, {
    files: qualityProject([], c.files),
    scripts: (project) => ({
      [c.id]: [{ tool: "write", args: { filePath: join(project, c.file), content: c.content } }],
    }),
  });
  c.prepare?.(s);
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(ctx.bin, s, ["--print-logs", `PROBE:${c.id}`], SMOKE_RUN_TIMEOUT_MS);
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    bytesChanged: s.exists(c.file) && s.sb.read(c.file) === c.content,
    noGate: qualityGate(s) === null,
    noDiagnostic: !messages.some((message) => message.includes("QUALITY VIOLATION")),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}
