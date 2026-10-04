/** What the language-quality pinned-host smokes share: project files, gate reads, the edit, patch and disabled cases. */
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

/** `text` as `+` lines of an `*** Add File` section. */
export function addedLines(text: string): string[] {
  return text
    .trimEnd()
    .split("\n")
    .map((line) => `+${line}`);
}

/** Runs scripted probe `id` on the pinned host and reads the tool states and tool messages. */
async function runProbe(ctx: ScenarioContext, s: Session, id: string) {
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(ctx.bin, s, ["--print-logs", `PROBE:${id}`], SMOKE_RUN_TIMEOUT_MS);
  return { host, states: toolStates(host), messages: finalMessages(s, "tool") };
}

type EditCase = {
  readonly id: string;
  readonly project: Record<string, string>;
  /** Language name for the bash call descriptions. */
  readonly language: string;
  readonly file: string;
  readonly bad: string;
  readonly clean: string;
  /** Text the bad write's diagnostic must contain. */
  readonly diagnostic: string;
};

/** Invalid write, refused commit and push, clean edit, then the bad text in an unchecked `notes.md`. */
export async function editQuality(ctx: ScenarioContext, c: EditCase) {
  using s = session(ctx, {
    files: c.project,
    scripts: (project) => ({
      [c.id]: [
        { tool: "write", args: { filePath: join(project, c.file), content: c.bad } },
        {
          tool: "bash",
          args: {
            command: "touch commit-marker && git commit --allow-empty -m 'fix: invalid'",
            description: `commit after invalid ${c.language} edit`,
          },
        },
        {
          tool: "bash",
          args: {
            command: "touch push-marker && git push origin main",
            description: `push after invalid ${c.language} edit`,
          },
        },
        {
          tool: "edit",
          args: { filePath: join(project, c.file), oldString: c.bad, newString: c.clean },
        },
        { tool: "write", args: { filePath: join(project, "notes.md"), content: c.bad } },
      ],
    }),
  });
  const { host, states, messages } = await runProbe(ctx, s, c.id);
  const deniedBash = states.filter((state) => state.tool === "bash" && state.status === "error");
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    diagnosticVisible: messages.some((message) => message.includes(c.diagnostic)),
    commitDenied:
      deniedBash.length === 2 && /quality gate failing/i.test(deniedBash[0]?.error ?? ""),
    pushDenied: /quality gate failing/i.test(deniedBash[1]?.error ?? ""),
    markersAbsent: !s.exists("commit-marker") && !s.exists("push-marker"),
    editCompleted: states.some((state) => state.tool === "edit" && state.status === "completed"),
    recovered: qualityGate(s)?.status === "passing" && s.sb.read(c.file) === c.clean,
    unrelatedIgnored:
      s.sb.read("notes.md") === c.bad && !(messages.at(-1)?.includes("QUALITY VIOLATION") ?? false),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

type ProjectFile = { readonly path: string; readonly content: string };

type PatchCase = {
  readonly id: string;
  readonly project: Record<string, string>;
  /** The gate entry source the module writes. */
  readonly source: string;
  /** Adds the failing `moved.from` and `removed` files. */
  readonly seed: (project: string) => string;
  /** Moves, deletes and adds in one patch. */
  readonly patch: (project: string) => string;
  readonly moved: { readonly from: string; readonly to: string; readonly content: string };
  readonly removed: string;
  /** A violating added file that must stay failing. */
  readonly added: ProjectFile;
  /** An added file of another type that must not be checked. */
  readonly notes: ProjectFile;
  /** Texts one seed diagnostic must all contain. */
  readonly seedDiagnostics: (project: string) => string[];
  /** Texts one patch diagnostic must all contain. */
  readonly patchDiagnostics: (project: string) => string[];
};

/** Seed two failing files, then one move/delete/add patch that leaves exactly two violations. */
export async function patchQuality(ctx: ScenarioContext, c: PatchCase) {
  using s = session(ctx, {
    model: "gpt-5-probe",
    files: c.project,
    scripts: (project) => ({
      [c.id]: [
        { tool: "apply_patch", args: { patchText: c.seed(project) } },
        { tool: "apply_patch", args: { patchText: c.patch(project) } },
      ],
    }),
  });
  const { host, states, messages } = await runProbe(ctx, s, c.id);
  const project = s.sb.project;
  const entries = qualityGate(s)?.entries ?? {};
  const expected = [join(project, c.moved.to), join(project, c.added.path)].toSorted();
  const allIn = (texts: string[]) =>
    messages.some((message) => texts.every((text) => message.includes(text)));
  const observed = {
    seedFailed: allIn(c.seedDiagnostics(project)),
    patchCompleted: states.some(
      (state) => state.tool === "apply_patch" && state.status === "completed",
    ),
    moveApplied:
      !s.exists(c.moved.from) && s.exists(c.moved.to) && s.sb.read(c.moved.to) === c.moved.content,
    deleteApplied: !s.exists(c.removed),
    addApplied: s.exists(c.added.path) && s.sb.read(c.added.path) === c.added.content,
    unrelatedApplied: s.exists(c.notes.path) && s.sb.read(c.notes.path) === c.notes.content,
    exactGateEntries:
      qualityGate(s)?.status === "failing" &&
      JSON.stringify(Object.keys(entries).toSorted()) === JSON.stringify(expected) &&
      expected.every((path) => entries[path]?.source === c.source),
    bothVisible: allIn(c.patchDiagnostics(project)),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
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
  const { host, states, messages } = await runProbe(ctx, s, c.id);
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    bytesChanged: s.exists(c.file) && s.sb.read(c.file) === c.content,
    noGate: qualityGate(s) === null,
    noDiagnostic: !messages.some((message) => message.includes("QUALITY VIOLATION")),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}
