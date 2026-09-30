/**
 * push-review (#262): a `git push` needs a recorded, clean code review of the
 * exact diff being pushed. A port of `pre-tools/modules/push-review.sh`. How
 * firmly a failed check lands is the `pushReview` gate mode (advise at the
 * shipped preset); in `ask` mode the question is recorded as a pending waiver
 * for that diff, which a push of it promotes (`push-waiver`, #259). Every check
 * reads the repository the push targets, so `git -C <worktree> push` is judged
 * on the worktree's branch, diff and state file (#283 item 9).
 */
import { existsSync, statSync } from "node:fs";
import { gateDecision, gateMode, type GateMode } from "../config/gate-mode.ts";
import type { Decision } from "../decision/decision.ts";
import { baseBranch, branchSlug } from "../detect/detect-branch.ts";
import { envValue } from "../host/host-name.ts";
import { projectStateDir } from "../host/host-roots.ts";
import { readJson } from "../ledger/verdict-gates.ts";
import { pushWaiverMatches, pushWaiverPend } from "../ledger/push-waiver.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { diffSha } from "../state/diff-sha.ts";
import { telemetryAppend } from "../state/telemetry.ts";
import {
  ALLOW,
  gateConfig,
  preToolHostEvent,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";
import { stateFailure, stateRound } from "./push-review-state.ts";
import { changedNames, pushTarget, refExists } from "./push-target.ts";

/** What `git hash-object --stdin` prints for an empty stream: never a reviewable diff. */
const EMPTY_BLOB_SHA = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

const ASK_LEAD =
  "No clean review is recorded for this diff. Approving pushes anyway, and the approval is remembered until the diff changes.";

type Push = {
  ctx: RegistryContext;
  mode: Exclude<GateMode, "off">;
  root: string;
  slug: string;
  base: string;
};

/** `_pr_telemetry RESULT CODE [ROUND]`: a round that is not all digits is null. */
function record(push: Push, result: string, code: string, round = ""): void {
  const value = /^[0-9]+$/.test(round) ? Number(round) : null;
  const options = { env: push.ctx.env, host: push.ctx.host };
  telemetryAppend(push.root, "push_check", { result, reason_code: code, round: value }, options);
}

/**
 * `_pr_decide CODE REASON [ROUND]`: one failed check in the configured mode.
 * An `ask` also records the pending waiver for `sha` when there is one.
 */
function decide(push: Push, code: string, reason: string, round = "", sha = ""): Decision {
  const { ctx, mode } = push;
  let text = reason;
  if (mode === "ask") {
    text = `${ASK_LEAD}\n\n${reason}`;
    if (sha !== "") {
      const options = { env: ctx.env, host: ctx.host };
      pushWaiverPend(push.root, push.slug, sha, push.base, code, options);
    }
  }
  record(push, mode === "block" ? "deny" : mode, code, round);
  return gateDecision(mode, text) ?? ALLOW;
}

const HINT =
  'the built-in `/code-review xhigh --fix` skill, recorded as "code-review" (or the `toolu-review:review` skill)';

function noStateReason(sha: string, base: string, file: string): string {
  return `Code review required before push (diff SHA ${sha}, base ${base}).
Run a code reviewer on \`git diff ${base}...HEAD\` and apply its findings — use ${HINT}. Then atomically write ${file} (tmp+mv) with schema { version: 2, branch, diff_sha, base_branch, reviewed_at, reviewers, findings_count, findings, review_round, reviewed_files }. \`reviewers\` must include at least one accepted reviewer (code-review, toolu-review:review, code-review:xhigh, review, or security-review), \`findings_count\` must be 0, \`review_round\` starts at 1 for a new \`diff_sha\` and bumps by 1 only when rewriting at the same \`diff_sha\`. \`reviewed_files\` must list every path from \`git diff ${base}...HEAD --name-only\` (sorted, unique) — the actual reviewer file coverage. Retry push.`;
}

function isFile(path: string): boolean {
  return existsSync(path) && statSync(path).isFile();
}

/** The review checks, once the push has a branch to key the state on. */
function reviewed(push: Push, branch: string, file: string): Decision {
  const { ctx, root, base } = push;
  if (!refExists(root, base, ctx.env)) {
    return decide(
      push,
      "base-missing",
      `base branch '${base}' not found locally; run \`git fetch origin ${base}:${base}\``,
    );
  }
  if (branch === "HEAD" || branch === "") {
    return decide(
      push,
      "detached-head",
      "detached HEAD — checkout a branch, or push an explicit `HEAD:<branch>` refspec so the review state can be keyed to that branch",
    );
  }
  if (branch === base) return ALLOW;
  const computed = diffSha(root, base, { env: ctx.env });
  if (computed === undefined) {
    record(push, "allow", "diff-failed");
    return ALLOW;
  }
  if (computed === EMPTY_BLOB_SHA) {
    const reason = `Refusing to push: diff against ${base} is empty. Either no commits diverged from base, or the branch was force-reset. Verify intent before pushing.`;
    return decide(push, "empty-diff", reason, "", "empty-diff");
  }
  const waiverOptions = { env: ctx.env, host: ctx.host };
  if (pushWaiverMatches(root, push.slug, computed, waiverOptions)) {
    record(push, "allow", "waived");
    return ALLOW;
  }
  if (!isFile(file)) {
    return decide(push, "no-state", noStateReason(computed, base, file), "", computed);
  }
  const doc = readJson(file) ?? null;
  const failure = stateFailure(doc, file, computed, base, changedNames(root, base, ctx.env));
  if (failure !== undefined) {
    return decide(push, failure.code, failure.reason, failure.round, computed);
  }
  record(push, "allow", "pass", stateRound(doc));
  return ALLOW;
}

function evaluate(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  const target = pushTarget(event, ctx);
  if (target === undefined) return ALLOW;
  const mode = gateMode(gateConfig(ctx, options), "pushReview", {
    host: ctx.host,
    event: preToolHostEvent(event),
  });
  if (mode === "off") return ALLOW;
  const { root, branch } = target;
  const slug = branchSlug(branch);
  const dir =
    envValue(ctx.env, "STATE_DIR") ??
    projectStateDir("push-review", { env: ctx.env, host: ctx.host, root });
  const base = envValue(ctx.env, "PUSH_REVIEW_BASE") ?? baseBranch(root, ctx.env);
  return reviewed({ ctx, mode, root, slug, base }, branch, `${dir ?? ""}/${slug}.json`);
}

/** The native `push-review` built-in PreToolUse module. */
export function pushReviewModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "push-review",
    run: (event, ctx) => Promise.resolve(evaluate(event, ctx, options)),
  };
}
