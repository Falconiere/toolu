/**
 * AC-7 (#256): push waivers vs `push-waiver.sh`. Every sha is a real
 * `git diff | git hash-object` value. Each scenario plays one sequence of
 * operations four ways: all bash, all TypeScript, and alternating in both
 * orders. It snapshots the state directory (bytes and mode) and each
 * operation's result. All four must agree. Expiry is checked against both
 * sweepers on waiver files this module writes.
 */
import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { childEnv, run } from "@toolu/conformance/harness/spawn";
import { diffSha } from "../../state/diff-sha.ts";
import { sweepState } from "../../state/state-sweeper.ts";
import {
  pushWaiverMatches,
  pushWaiverPath,
  pushWaiverPend,
  pushWaiverPendingPath,
  pushWaiverPromote,
} from "../push-waiver.ts";
import { LIB, bashEval } from "./ledger-parity-helpers.ts";

type Op =
  | { op: "pend"; sha: "sha" | "new" | ""; base: string; code: string; slug?: string }
  | { op: "promote" | "matches"; sha: "sha" | "new" | ""; slug?: string }
  | { op: "paths" }
  | { op: "commit" }
  | { op: "raw"; file: "waiver" | "pending"; body: string };

type Impl = "bash" | "ts";
const SLUG = "feat_x";
const STAMP = /\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ/g;

function repo(): Sandbox {
  const sb = createSandbox({ git: true, files: { "a.txt": "one\n" } });
  sb.git("checkout", "-qb", "feat/x");
  sb.write("b.txt", "two\n");
  sb.git("add", "b.txt");
  sb.git("commit", "-qm", "feat: two");
  return sb;
}

async function apply(
  sb: Sandbox,
  impl: Impl,
  step: Op,
  shas: Record<string, string>,
  env: Record<string, string>,
) {
  const tsEnv = childEnv(env);
  if (step.op === "commit") {
    sb.write("c.txt", "three\n");
    sb.git("add", "c.txt");
    sb.git("commit", "-qm", "feat: three");
    shas.new = diffSha(sb.project, "main") ?? "";
    return "";
  }
  if (step.op === "raw") {
    const file =
      step.file === "waiver"
        ? pushWaiverPath(sb.project, SLUG, { env: tsEnv })
        : pushWaiverPendingPath(sb.project, SLUG, { env: tsEnv });
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, step.body.replace("SHA", shas.sha ?? ""));
    return "";
  }
  if (step.op === "paths") {
    if (impl === "ts") {
      return `${pushWaiverPath(sb.project, SLUG, { env: tsEnv })}|${pushWaiverPendingPath(sb.project, SLUG, { env: tsEnv })}`;
    }
    const res = await bashEval(
      "push-waiver.sh",
      'printf "%s|%s" "$(push_waiver_path "$1" "$2")" "$(push_waiver_pending_path "$1" "$2")"',
      [sb.project, SLUG],
      { cwd: sb.project, env },
    );
    return res.stdout;
  }
  const sha = step.sha === "" ? "" : (shas[step.sha] ?? "");
  const slug = step.slug ?? SLUG;
  if (impl === "ts") {
    const o = { env: tsEnv };
    if (step.op === "pend")
      return String(pushWaiverPend(sb.project, slug, sha, step.base, step.code, o));
    if (step.op === "promote") return String(pushWaiverPromote(sb.project, slug, sha, o));
    return String(pushWaiverMatches(sb.project, slug, sha, o));
  }
  const args =
    step.op === "pend" ? [sb.project, slug, sha, step.base, step.code] : [sb.project, slug, sha];
  const res = await bashEval("push-waiver.sh", `push_waiver_${step.op} "$@"`, args, {
    cwd: sb.project,
    env,
  });
  return String(res.exitCode === 0);
}

/** Every file under the project's state dirs: normalized bytes and mode. */
function snapshot(sb: Sandbox, dirs: string[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const dir of dirs) {
    if (!existsSync(dir)) continue;
    for (const name of readdirSync(dir).sort()) {
      const file = join(dir, name);
      const body = readFileSync(file, "utf8").replace(STAMP, "<T>");
      out[`${dir.replace(sb.root, "")}/${name}`] =
        `${(statSync(file).mode & 0o777).toString(8)} ${body}`;
    }
  }
  return out;
}

async function play(ops: Op[], impls: (i: number) => Impl, stateDir: boolean, host: string) {
  using sb = repo();
  const env: Record<string, string> = { HOME: sb.home, TOOLU_HOST_OVERRIDE: host };
  if (stateDir) env.STATE_DIR = join(sb.root, "state", "push-review");
  const shas: Record<string, string> = { sha: diffSha(sb.project, "main") ?? "" };
  const dirs = [
    join(sb.root, "state", "push-review"),
    join(sb.project, `.${host}`, "tmp", "push-review"),
  ];
  const trace: unknown[] = [];
  for (const [i, step] of ops.entries()) {
    const result = await apply(sb, impls(i), step, shas, env);
    trace.push({
      op: step.op,
      result: result.replaceAll(sb.root, "<ROOT>"),
      files: snapshot(sb, dirs),
    });
  }
  return trace;
}

const pend = (sha: "sha" | "new" | "" = "sha", code = "no-state", slug?: string): Op =>
  slug === undefined
    ? { op: "pend", sha, base: "main", code }
    : { op: "pend", sha, base: "main", code, slug };
const promote = (sha: "sha" | "new" | "" = "sha", slug?: string): Op =>
  slug === undefined ? { op: "promote", sha } : { op: "promote", sha, slug };
const matches = (sha: "sha" | "new" | "" = "sha"): Op => ({ op: "matches", sha });

const SCENARIOS: Record<string, Op[]> = {
  "no waiver before one is written": [matches()],
  "pend, promote, matches": [pend(), promote(), matches()],
  "promotion removes the pending marker and keeps the reason code": [
    pend("sha", "stale-diff"),
    promote(),
    matches(),
  ],
  "promotion refuses a marker from a different diff": [
    pend(),
    { op: "commit" },
    promote("new"),
    matches("new"),
  ],
  "a waiver stops matching once the diff changes": [
    pend(),
    promote(),
    { op: "commit" },
    matches("new"),
    matches(),
  ],
  "promotion with no pending marker writes nothing": [promote()],
  "a corrupt waiver reads as no waiver": [
    { op: "raw", file: "waiver", body: "not json" },
    matches(),
  ],
  "an unknown schema version reads as no waiver": [
    { op: "raw", file: "waiver", body: '{"version":99,"diff_sha":"SHA"}\n' },
    matches(),
  ],
  "a string version 1 still matches": [
    { op: "raw", file: "waiver", body: '{"version":"1","diff_sha":"SHA"}\n' },
    matches(),
  ],
  "an array waiver reads as no waiver": [{ op: "raw", file: "waiver", body: "[1]\n" }, matches()],
  "an empty sha never pends, promotes or matches": [
    pend(""),
    pend(),
    promote(),
    matches(""),
    promote(""),
  ],
  "pending markers are per branch": [pend(), promote("sha", "other_branch"), matches()],
  "asking again overwrites the earlier marker": [
    pend(),
    { op: "commit" },
    pend("new", "findings"),
    promote("new"),
    matches("new"),
  ],
  "a hand-written marker keeps its extra keys on promotion": [
    {
      op: "raw",
      file: "pending",
      body: '{"version":1,"diff_sha":"SHA","waived_at":"old","extra":[1],"asked_at":"x"}',
    },
    promote(),
    matches(),
  ],
  "waiver paths": [{ op: "paths" }],
};

describe("push waivers vs push-waiver.sh", () => {
  const mixes: Record<string, (i: number) => Impl> = {
    "ts only": () => "ts",
    "bash then ts": (i) => (i % 2 === 0 ? "bash" : "ts"),
    "ts then bash": (i) => (i % 2 === 0 ? "ts" : "bash"),
  };
  for (const [name, ops] of Object.entries(SCENARIOS)) {
    for (const [layout, stateDir, host] of [
      ["STATE_DIR", true, "claude"],
      ["project dir", false, "codex"],
    ] as const) {
      test.concurrent(`${name} (${layout})`, async () => {
        const plays = await Promise.all(
          [() => "bash" as const, ...Object.values(mixes)].map((impls) =>
            play(ops, impls, stateDir, host),
          ),
        );
        const [bash = [], ...others] = plays;
        const names = Object.keys(mixes);
        for (const [k, trace] of others.entries()) {
          expect({ mix: names[k] ?? "", trace }).toEqual({ mix: names[k] ?? "", trace: bash });
        }
      }, 60_000);
    }
  }
});

const HOUR = 3_600_000;

/** A live, unmerged, non-current branch whose waiver files the sweeper judges by age. */
function sweepRepo(): Sandbox {
  const sb = repo();
  for (const name of ["feat/old", "feat/fresh"]) {
    sb.git("checkout", "-q", "-b", name, "main");
    sb.write(`${name}.txt`, name);
    sb.git("add", "-A");
    sb.git("commit", "-qm", name);
  }
  sb.git("checkout", "-q", "feat/x");
  return sb;
}

async function arrangeAndSweep(impl: Impl): Promise<string[]> {
  using sb = sweepRepo();
  const env = { HOME: sb.home, TOOLU_HOST_OVERRIDE: "claude" };
  const o = { env: childEnv(env) };
  const sha = diffSha(sb.project, "main") ?? "";
  for (const slug of ["feat_old", "feat_fresh", "feat_x"]) {
    expect(pushWaiverPend(sb.project, slug, sha, "main", "no-state", o)).toBe(true);
    expect(pushWaiverPromote(sb.project, slug, sha, o)).toBe(true);
    expect(pushWaiverPend(sb.project, slug, sha, "main", "findings", o)).toBe(true);
  }
  const dir = join(sb.project, ".claude", "tmp", "push-review");
  const old = new Date(Date.now() - 24 * HOUR - 60_000);
  for (const name of [
    "feat_old.waiver.json",
    "feat_old.pending-waiver.json",
    "feat_x.waiver.json",
  ]) {
    utimesSync(join(dir, name), old, old);
  }
  if (impl === "bash") {
    const res = await run(
      [
        "bash",
        "-c",
        '. "$1"; toolu_sweep_state "$2"',
        "_",
        join(LIB, "state-sweeper.sh"),
        sb.project,
      ],
      { cwd: sb.project, env },
    );
    expect(res.exitCode).toBe(0);
  } else {
    sweepState(sb.project, { ...o, host: "claude", warn: () => {} });
  }
  return readdirSync(dir).sort();
}

test.concurrent("expiry: aged waivers of a live branch are swept by both sweepers, fresh and current ones kept", async () => {
  const bash = await arrangeAndSweep("bash");
  expect(bash).toEqual([
    "feat_fresh.pending-waiver.json",
    "feat_fresh.waiver.json",
    "feat_x.pending-waiver.json",
    "feat_x.waiver.json",
  ]);
  expect(await arrangeAndSweep("ts")).toEqual(bash);
});
