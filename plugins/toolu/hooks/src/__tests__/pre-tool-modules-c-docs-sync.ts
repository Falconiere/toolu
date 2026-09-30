/**
 * docs-sync cases (#262): every `@test` of the deleted docs-sync.bats,
 * docs-sync-mode.bats, the two `docs-sync docs_…` tests of telemetry-sites.bats
 * and the `parity (docs-sync.sh)` test of diff-sha.bats. Skipped: "module is
 * picked up by the modules/*.sh glob" (no glob any more). Each case runs the
 * whole hook with push-review and plan-ledger off, so only docs-sync decides.
 * The docs_nudge / docs_attested telemetry lines are recorded as touched files.
 */
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  BASE,
  commitFile,
  diffShaIn,
  FEATURE,
  gitIn,
  group,
  slugOf,
  writeIn,
  type CaseInput,
  type GateCase,
} from "./pre-tool-modules-c-cases.ts";

/** Push-review and plan-ledger off, plus the `docsSync` config the bats test wrote. */
const cfg = (docsSync?: object): object => ({
  version: 1,
  gates: { pushReview: { mode: "off" }, planLedger: { mode: "off" } },
  ...(docsSync === undefined ? {} : { docsSync }),
});

/** The bats `setup_sandbox`: base.txt on the base branch, then `feat/example` with the given files. */
const repoWith =
  (...files: [path: string, body: string][]) =>
  (sb: Sandbox): void => {
    commitFile(sb, "base.txt", "base");
    gitIn(sb.project, ["checkout", "-q", "-b", FEATURE]);
    for (const [path, body] of files) commitFile(sb, path, body);
  };

const CODE: [string, string] = ["lib/foo.sh", "echo hi"];

/** An attestation for the diff as it stands, at the default state dir of the claude host. */
function attest(sb: Sandbox, sha: string = diffShaIn(sb.project), decision = "not-needed"): void {
  writeIn(
    sb.project,
    `.claude/tmp/docs-sync/${slugOf(FEATURE)}.json`,
    JSON.stringify({
      version: 1,
      branch: FEATURE,
      diff_sha: sha,
      base_branch: BASE,
      decision,
      note: "covered by test",
      attested_at: "2026-06-15T00:00:00Z",
    }),
  );
}

const ds = group({ config: cfg(), command: "git push" });

const nudge = (c: Omit<CaseInput, "expect">): GateCase =>
  ds({ expect: "advisory", has: ["docs-sync"], ...c });
const silent = (c: Omit<CaseInput, "expect">): GateCase => ds({ expect: "silent", ...c });

const CODE_ONLY = repoWith(CODE);

const ADVISE_CASES: GateCase[] = [
  silent({
    name: "docs-sync: non-Bash tool exits silently",
    setup: CODE_ONLY,
    fixture: (sb) => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "Edit",
      toolInput: { file_path: sb.path("notes.txt"), old_string: "a", new_string: "b" },
    }),
  }),
  silent({
    name: "docs-sync: Bash but not git push exits silently",
    setup: CODE_ONLY,
    command: "ls -la",
  }),
  silent({
    name: "docs-sync: empty diff against base is a silent no-op",
    setup: repoWith(),
  }),
  silent({
    name: "docs-sync: detached HEAD is a silent no-op",
    setup: (sb) => {
      CODE_ONLY(sb);
      gitIn(sb.project, ["checkout", "-q", gitIn(sb.project, ["rev-parse", "HEAD"]).trim()]);
    },
  }),
  silent({
    name: "docs-sync: missing base branch is a silent no-op",
    setup: CODE_ONLY,
    env: () => ({ DOCS_SYNC_BASE: "nope" }),
  }),
  nudge({
    name: "docs-sync: code change with no doc surface fires an advisory",
    setup: CODE_ONLY,
  }),
  nudge({
    // bats also asserted `hookEventName == "PreToolUse"`; the advisory shape is in the golden.
    name: "docs-sync: advisory is never a blocking decision (advisory-only invariant)",
    setup: CODE_ONLY,
  }),
  silent({
    name: "docs-sync: code change WITH a README touch is silent",
    setup: repoWith(CODE, ["README.md", "# updated"]),
  }),
  silent({
    name: "docs-sync: code change WITH a nested docs/ guide touch is silent",
    setup: repoWith(CODE, ["docs/guide/usage.md", "how to"]),
  }),
  silent({
    name: "docs-sync: code change WITH a SKILL.md trigger touch is silent",
    setup: repoWith(CODE, ["skills/thing/SKILL.md", "trigger text"]),
  }),
  nudge({
    name: "docs-sync: code change with ONLY a release-note touch still fires",
    setup: repoWith(CODE, ["docs/releases/v9.9.0.md", "notes"]),
  }),
  silent({
    name: "docs-sync: a fresh diff-sha attestation silences the advisory",
    setup: (sb) => {
      CODE_ONLY(sb);
      attest(sb);
    },
  }),
  nudge({
    name: "docs-sync: a stale attestation (diff changed) does NOT silence it",
    setup: (sb) => {
      CODE_ONLY(sb);
      attest(sb);
      commitFile(sb, "lib/bar.sh", "echo bye");
    },
  }),
  nudge({
    name: "docs-sync: a corrupted attestation file falls safe (still fires)",
    setup: (sb) => {
      CODE_ONLY(sb);
      writeIn(sb.project, `.claude/tmp/docs-sync/${slugOf(FEATURE)}.json`, "not json at all");
    },
  }),
  silent({
    name: "docs-sync: a project surfaces override is honored",
    config: cfg({ surfaces: ["wiki/*.md"] }),
    setup: repoWith(CODE, ["wiki/page.md", "doc"]),
  }),
  nudge({
    name: "docs-sync: under a surfaces override, README no longer counts",
    config: cfg({ surfaces: ["wiki/*.md"] }),
    setup: repoWith(CODE, ["README.md", "# updated"]),
  }),
  silent({
    name: "docs-sync: a doc-only change (no code) is silent",
    setup: repoWith(["README.md", "# updated"]),
  }),
];

const MODE_CASES: GateCase[] = [
  ds({
    name: "docs-sync: mode=block: code-without-doc, no attestation -> push denied, reason mentions docs",
    config: cfg({ mode: "block" }),
    setup: CODE_ONLY,
    expect: "deny",
    has: ["docs-sync", "attest"],
  }),
  nudge({
    name: "docs-sync: default mode (advise): allow + advisory context + docs_nudge telemetry",
    setup: CODE_ONLY,
  }),
  silent({
    name: "docs-sync: mode=block: valid attestation (module's own path resolution) -> allow + docs_attested",
    config: cfg({ mode: "block" }),
    setup: (sb) => {
      CODE_ONLY(sb);
      attest(sb);
    },
  }),
  silent({
    name: "docs-sync: mode=block: doc file present in diff -> allow, no output, no deny",
    config: cfg({ mode: "block" }),
    setup: repoWith(CODE, ["README.md", "# updated"]),
  }),
  silent({
    name: "docs-sync: mode=off: allow, no advisory, no telemetry file",
    config: cfg({ mode: "off" }),
    setup: CODE_ONLY,
  }),
  silent({
    name: "docs-sync: mode=off: valid attestation present still emits NO telemetry (off means off)",
    config: cfg({ mode: "off" }),
    setup: (sb) => {
      CODE_ONLY(sb);
      attest(sb);
    },
  }),
  // The bats test asserted the `docsSync.mode ... is not an allowed value` warning on the
  // module's stderr; the dispatcher prints `toolu-config:` warnings itself, and the golden
  // records the whole stderr.
  nudge({
    name: "docs-sync: mode=junk: warns on stderr and falls back to advise behavior",
    config: cfg({ mode: "junk" }),
    setup: CODE_ONLY,
  }),
];

const TELEMETRY_CASES: GateCase[] = [
  nudge({
    name: "docs-sync: docs_nudge: code change without a doc surface fires the advisory event",
    setup: CODE_ONLY,
  }),
  silent({
    name: "docs-sync: docs_attested: a matching attestation fires docs_attested with its decision",
    setup: (sb) => {
      CODE_ONLY(sb);
      attest(sb, diffShaIn(sb.project), "not-needed");
    },
  }),
  silent({
    name: "docs-sync: parity (docs-sync.sh): attestation keyed to the raw-formula sha still silences the nudge",
    setup: (sb) => {
      CODE_ONLY(sb);
      attest(sb, diffShaIn(sb.project, BASE));
    },
  }),
];

export const DOCS_SYNC_CASES: GateCase[] = [...ADVISE_CASES, ...MODE_CASES, ...TELEMETRY_CASES];
