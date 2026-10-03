/** Pinned-host proof of selected Python quality checks on native OpenCode tools (#353). */
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";
import { session, SMOKE_RUN_TIMEOUT_MS, type PretoolScenario } from "./pretool-shared.ts";
import { disabledQuality, qualityGate, qualityProject } from "./quality-smoke-shared.ts";
import { prepareSdk, verdict } from "./scenarios-posttool-smoke.ts";

const FILES = { "pyproject.toml": '[project]\nname = "python-quality-smoke"\n' };
const PROJECT = qualityProject(["python-quality"], FILES);
const BARE = "def load():\n    try:\n        return 1\n    except:\n        return 0\n";
const SWALLOW = "try:\n    run()\nexcept Exception: pass\n";
const MOCKED = "from unittest import mock\n\n\ndef test_it():\n    assert mock\n";
const CLEAN = '"""Clean."""\n\n\ndef answer():\n    """Answer."""\n    return 42\n';

/** `text` as `+` lines of an `*** Add File` section. */
function added(text: string): string[] {
  return text
    .trimEnd()
    .split("\n")
    .map((line) => `+${line}`);
}

async function editQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: PROJECT,
    scripts: (project) => ({
      "pyquality.edit": [
        { tool: "write", args: { filePath: join(project, "bad.py"), content: SWALLOW } },
        {
          tool: "bash",
          args: {
            command: "touch commit-marker && git commit --allow-empty -m 'fix: invalid'",
            description: "commit after invalid Python edit",
          },
        },
        {
          tool: "bash",
          args: {
            command: "touch push-marker && git push origin main",
            description: "push after invalid Python edit",
          },
        },
        {
          tool: "edit",
          args: { filePath: join(project, "bad.py"), oldString: SWALLOW, newString: CLEAN },
        },
        { tool: "write", args: { filePath: join(project, "notes.md"), content: SWALLOW } },
      ],
    }),
  });
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pyquality.edit"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const deniedBash = states.filter((state) => state.tool === "bash" && state.status === "error");
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    diagnosticVisible: messages.some((message) => message.includes("Forbidden suppression")),
    commitDenied:
      deniedBash.length === 2 && /quality gate failing/i.test(deniedBash[0]?.error ?? ""),
    pushDenied: /quality gate failing/i.test(deniedBash[1]?.error ?? ""),
    markersAbsent: !s.exists("commit-marker") && !s.exists("push-marker"),
    editCompleted: states.some((state) => state.tool === "edit" && state.status === "completed"),
    recovered: qualityGate(s)?.status === "passing" && s.sb.read("bad.py") === CLEAN,
    unrelatedIgnored:
      s.sb.read("notes.md") === SWALLOW &&
      !(messages.at(-1)?.includes("QUALITY VIOLATION") ?? false),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

function seedPatch(project: string): string {
  return [
    "*** Begin Patch",
    `*** Add File: ${join(project, "pkg/old.py")}`,
    ...added(BARE),
    `*** Add File: ${join(project, "pkg/removed.py")}`,
    ...added(BARE),
    "*** End Patch",
  ].join("\n");
}

function patchText(project: string): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${join(project, "pkg/old.py")}`,
    `*** Move to: ${join(project, "pkg/moved.py")}`,
    "@@",
    "     try:",
    "         return 1",
    "-    except:",
    "-        return 0",
    "+    except Exception: pass",
    `*** Delete File: ${join(project, "pkg/removed.py")}`,
    `*** Add File: ${join(project, "pkg/test_added.py")}`,
    ...added(MOCKED),
    `*** Add File: ${join(project, "pkg/notes.md")}`,
    "+except: pass",
    "*** End Patch",
  ].join("\n");
}

async function patchQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    model: "gpt-5-probe",
    files: PROJECT,
    scripts: (project) => ({
      "pyquality.patch": [
        { tool: "apply_patch", args: { patchText: seedPatch(project) } },
        { tool: "apply_patch", args: { patchText: patchText(project) } },
      ],
    }),
  });
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pyquality.patch"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const entries = qualityGate(s)?.entries ?? {};
  const expected = [
    join(s.sb.project, "pkg/moved.py"),
    join(s.sb.project, "pkg/test_added.py"),
  ].toSorted();
  const moved = BARE.replace("    except:\n        return 0\n", "    except Exception: pass\n");
  const observed = {
    seedFailed: messages.some(
      (message) =>
        message.includes(`${join(s.sb.project, "pkg/old.py")} `) &&
        message.includes(`${join(s.sb.project, "pkg/removed.py")} `),
    ),
    patchCompleted: states.some(
      (state) => state.tool === "apply_patch" && state.status === "completed",
    ),
    moveApplied:
      !s.exists("pkg/old.py") && s.exists("pkg/moved.py") && s.sb.read("pkg/moved.py") === moved,
    deleteApplied: !s.exists("pkg/removed.py"),
    addApplied: s.exists("pkg/test_added.py") && s.sb.read("pkg/test_added.py") === MOCKED,
    unrelatedApplied: s.exists("pkg/notes.md") && s.sb.read("pkg/notes.md") === "except: pass\n",
    exactGateEntries:
      qualityGate(s)?.status === "failing" &&
      JSON.stringify(Object.keys(entries).toSorted()) === JSON.stringify(expected) &&
      expected.every((path) => entries[path]?.source === "python-quality-hook"),
    bothVisible: messages.some(
      (message) =>
        message.includes("one-line except ...: pass") && message.includes("no-mocks: mock import"),
    ),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

export const PYTHON_QUALITY_SCENARIOS: PretoolScenario[] = [
  { id: "pyquality.edit", run: editQuality },
  { id: "pyquality.patch", run: patchQuality },
  {
    id: "pyquality.disabled",
    run: (ctx) =>
      disabledQuality(ctx, {
        id: "pyquality.disabled",
        files: FILES,
        file: "bad.py",
        content: SWALLOW,
      }),
  },
];
