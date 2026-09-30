/**
 * The review and docs push gates as `verdict.sh` reads them (#256). The review
 * gate applies the push-review v2 schema directly to the state file, with a
 * closed `reason_code` set. The docs gate mirrors docs-sync's surface
 * classification and its attestation lookup.
 */
import { loadConfig } from "../config/config-load.ts";
import { configString } from "../config/config-read.ts";
import {
  docsSyncCodeSurfaces,
  docsSyncSurfaceExcludes,
  docsSyncSurfaces,
} from "../config/docs-sync-config.ts";
import { envValue } from "../host/host-name.ts";
import { projectStateDir, resolveHost } from "../host/host-roots.ts";
import { diffSha } from "../state/diff-sha.ts";
import { baseBranch, branchSlug } from "../state/state-git.ts";
import { matchesAny } from "./glob.ts";
import { JqError, alt, get, toStr, type Json } from "./ledger-jq.ts";
import { isFile } from "./ledger-parse.ts";
import { hasAcceptedReviewer, outputLines, reviewedFiles, sortedUnique } from "./review-state.ts";
import {
  gate,
  git,
  rawOr,
  readJson,
  stateDir,
  type Gate,
  type GateContext,
} from "./verdict-gates.ts";

const EMPTY_BLOB_SHA = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
const NO_ROUND = { reason_code: null, round: null };

/** `(.review_round // 1) | if type == "number" then . else null end`. */
function roundOf(state: Json): Json {
  try {
    const round = alt(get(state, "review_round"), 1);
    return typeof round === "number" ? round : null;
  } catch (error) {
    if (error instanceof JqError) return null;
    throw error;
  }
}

/** The v2 checks after the state parses: schema, reviewer, round cap, staleness, coverage, findings. */
function reviewState(ctx: GateContext, state: Json, file: string): Gate {
  const round = roundOf(state);
  const code = (reasonCode: string) => ({ reason_code: reasonCode, round });
  const version = rawOr(() => toStr(alt(get(state, "version"), "")));
  const sha = rawOr(() => alt(get(state, "diff_sha"), ""));
  const findings = rawOr(() => toStr(alt(get(state, "findings_count"), "")));
  if (version === "1") {
    return gate(
      "fail",
      "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file",
      code("schema-v1"),
    );
  }
  if (version !== "2" || sha === "" || findings === "") {
    return gate(
      "fail",
      `push-review state file is corrupted or missing required v2 fields at ${file}`,
      code("schema"),
    );
  }
  if (!hasAcceptedReviewer(state))
    return gate("fail", "state file lists no accepted reviewer", code("reviewer"));
  const whole = typeof round === "number" ? String(Math.floor(round)) : "";
  if (/^-?\d+$/.test(whole) && BigInt(whole) > 5n) {
    return gate(
      "escalate",
      `review loop hit ${whole} rounds (max 5) on an unchanged diff`,
      code("round-cap"),
    );
  }
  if (sha !== ctx.cur)
    return gate("fail", "diff changed since review (state stale)", code("stale-diff"));
  const changed = outputLines(
    git(ctx, ["diff", "--no-color", `${ctx.base}...HEAD`, "--name-only"]) ?? "",
  );
  if (sortedUnique(changed) !== sortedUnique(reviewedFiles(state))) {
    return gate(
      "fail",
      "reviewed_files does not match the current diff's changed paths",
      code("file-coverage"),
    );
  }
  if (findings !== "0")
    return gate("fail", `code review has open findings (${findings})`, code("findings"));
  return gate("pass", "review satisfied", code("pass"));
}

/** `vd_gate_review`: the push-review v2 state against the current diff. */
export function reviewGate(ctx: GateContext): Gate {
  if (ctx.branch === ctx.base) return gate("skip", "current branch is the base branch", NO_ROUND);
  if (ctx.cur === "") return gate("skip", `could not compute diff against ${ctx.base}`, NO_ROUND);
  if (ctx.cur === EMPTY_BLOB_SHA) {
    return gate("fail", `diff against ${ctx.base} is empty; verify intent before pushing`, {
      reason_code: "empty-diff",
      round: null,
    });
  }
  const file = `${stateDir(ctx, "push-review", "STATE_DIR")}/${branchSlug(ctx.branch)}.json`;
  if (!isFile(file)) {
    return gate("fail", "no push-review state file; run a reviewer and write the state", {
      reason_code: "no-state",
      round: null,
    });
  }
  const state = readJson(file);
  if (state === undefined || state === null || state === false) {
    return gate("fail", `push-review state file is unparseable at ${file}`, {
      reason_code: "schema",
      round: null,
    });
  }
  return reviewState(ctx, state, file);
}

/** `vd_gate_docs`: code changed without a doc surface, and no attestation for this diff. */
export function docsGate(ctx: GateContext): Gate {
  const base = envValue(ctx.env, "DOCS_SYNC_BASE") ?? baseBranch(ctx.root, ctx.env);
  const host = resolveHost(
    ctx.host === undefined ? { env: ctx.env } : { env: ctx.env, host: ctx.host },
  );
  const config = loadConfig({ ...host, cwd: ctx.cwd, warn: ctx.warn });
  const mode = configString(config, "docsSync.mode", "advise", ["advise", "block", "off"]);
  if (mode === "off") return gate("skip", "docsSync.mode is off");
  if (ctx.branch === "" || ctx.branch === "HEAD") return gate("skip", "detached HEAD");
  if (git(ctx, ["rev-parse", "--verify", "--quiet", base]) === undefined) {
    return gate("skip", `base branch '${base}' not found locally`);
  }
  if (ctx.branch === base) return gate("skip", "current branch is the base branch");
  const changed = (git(ctx, ["diff", "--no-color", `${base}...HEAD`, "--name-only"]) ?? "")
    .split("\n")
    .filter((path) => path !== "");
  if (changed.length === 0) return gate("skip", `no diff against ${base}`);
  const surfaces = docsSyncSurfaces(config);
  const excludes = docsSyncSurfaceExcludes(config);
  const code = docsSyncCodeSurfaces(config);
  const hasDoc = changed.some((p) => matchesAny(p, surfaces) && !matchesAny(p, excludes));
  const hasCode = changed.some((p) => matchesAny(p, code));
  if (!hasCode || hasDoc) return gate("pass", "doc surface in sync");
  const sha = diffSha(ctx.root, base, { env: ctx.env }) ?? "";
  const dir =
    envValue(ctx.env, "DOCS_SYNC_STATE_DIR") ??
    projectStateDir("docs-sync", { ...host, cwd: ctx.cwd }) ??
    "";
  const attestation = `${dir}/${branchSlug(ctx.branch)}.json`;
  if (isFile(attestation) && sha !== "") {
    const attested = rawOr(() => alt(get(readJson(attestation) ?? null, "diff_sha"), ""));
    if (attested === sha) return gate("pass", "doc change attested as not needed");
  }
  return mode === "block"
    ? gate("fail", "code changed without a doc update (docsSync.mode=block)")
    : gate("advise", "code changed without a doc update");
}
