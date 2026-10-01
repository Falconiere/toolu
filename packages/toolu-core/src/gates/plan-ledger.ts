/**
 * plan-ledger (#262): a `git push` waits until every plan-ledger step is
 * fresh-green, i.e. green at the current branch diff, and the ledger was
 * verified against that whole diff. Read-only: it never runs a step. A port of
 * `pre-tools/modules/plan-ledger.sh`; the ledger is the pushed repository's
 * (#283 item 9). An absent ledger never blocks.
 */
import { gateDecision, gateMode, type GateMode } from "../config/gate-mode.ts";
import type { Decision } from "../decision/decision.ts";
import { baseBranch, branchSlug } from "../detect/detect-branch.ts";
import { envValue } from "../host/host-name.ts";
import { projectStateDir } from "../host/host-roots.ts";
import { readLedger } from "../ledger/ledger-io.ts";
import { isFile } from "../ledger/ledger-parse.ts";
import {
  alt,
  concat,
  each,
  get,
  isObject,
  JqError,
  jqEquals,
  length,
  type Json,
} from "../ledger/ledger-jq.ts";
import { rawOr } from "../ledger/verdict-gates.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { diffSha } from "../state/diff-sha.ts";
import {
  ALLOW,
  gateConfig,
  preToolHostEvent,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";
import { acBlockers } from "./plan-ledger-ac.ts";
import { gitAt, pushTarget } from "./push-target.ts";

const RUN = 'bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run';

/** `id: effective — title` for each step that is not fresh-green; a jq error keeps the lines before it. */
function blockers(ledger: Json, cur: string): string {
  const lines: string[] = [];
  try {
    for (const step of each(get(ledger, "steps"))) {
      const status = get(step, "status");
      const green = jqEquals(status, "green");
      const eff: Json =
        green && jqEquals(get(step, "diff_sha"), cur)
          ? "green"
          : green
            ? "stale"
            : alt(status, "pending");
      if (jqEquals(eff, "green")) continue;
      lines.push(concat(alt(get(step, "id"), "?"), ": ", eff, " — ", alt(get(step, "title"), "")));
    }
  } catch (error) {
    if (!(error instanceof JqError)) throw error;
  }
  return lines.join("\n").replace(/\n+$/, "");
}

type Judged = { mode: Exclude<GateMode, "off">; ledger: Json; cur: string; hint: string };

/** The step and verify checks, once the ledger parsed at schema v1 with steps. */
function stepsDecision({ mode, ledger, cur, hint }: Judged): Decision {
  const blocked = blockers(ledger, cur);
  const verified = rawOr(() => alt(get(ledger, "verified_sha"), ""));
  if (
    blocked === "" &&
    isObject(ledger) &&
    Object.hasOwn(ledger, "verified_sha") &&
    verified !== cur
  ) {
    const reason = `plan-ledger: every step is green, but not verified against the current diff.\n\nScoped runs judge a step on its own \`paths\`; the push gate judges it on the whole branch.\n\nrun: ${RUN} ${hint} --verify`;
    return gateDecision(mode, reason) ?? ALLOW;
  }
  if (blocked === "") return ALLOW;
  const reason = `plan-ledger: push blocked — steps not fresh-green:\n${blocked}\n\nrun: ${RUN} ${hint}`;
  return gateDecision(mode, reason) ?? ALLOW;
}

function evaluate(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  const target = pushTarget(event, ctx);
  if (target === undefined) return ALLOW;
  const config = gateConfig(ctx, options);
  const mode = gateMode(config, "planLedger", { host: ctx.host, event: preToolHostEvent(event) });
  if (mode === "off") return ALLOW;
  const { root } = target;
  const base = envValue(ctx.env, "PUSH_REVIEW_BASE") ?? baseBranch(root, ctx.env);
  const branch =
    target.branch !== ""
      ? target.branch
      : (gitAt(root, ["rev-parse", "--abbrev-ref", "HEAD"], ctx.env) ?? "").trim();
  const dir =
    envValue(ctx.env, "LEDGER_DIR") ??
    projectStateDir("plan-ledger", { env: ctx.env, host: ctx.host, root });
  const file = `${dir ?? ""}/${branchSlug(branch)}.json`;
  // Absent: never a deny (bash's stderr nudge never reached the host).
  if (!isFile(file)) return ALLOW;
  const read = readLedger(file);
  if (read === undefined) {
    return {
      kind: "deny",
      reason: `plan-ledger: unparseable ledger at ${file}; delete and re-run \`${RUN} <plan_doc>\``,
    };
  }
  const ledger = read.value;
  const version = rawOr(() => alt(get(ledger, "version"), ""));
  if (version !== "1") {
    const reason = `plan-ledger: ledger schema mismatch at ${file} (version="${version}", expected 1); delete and re-run \`${RUN} <plan_doc>\``;
    return gateDecision(mode, reason) ?? ALLOW;
  }
  const total = rawOr(() => alt(get(get(ledger, "summary"), "total"), 0), "0");
  const count = rawOr(() => length(get(ledger, "steps")), "0");
  if (total === "0" || count === "0") return ALLOW;
  const cur = diffSha(root, base, { env: ctx.env });
  if (cur === undefined) return ALLOW;
  const uncovered = acBlockers(ledger, cur, root, ctx, config);
  if (uncovered !== undefined) {
    const reason = `plan-ledger: push blocked — uncovered spec AC id(s):\n${uncovered}\n\ncover with a fresh-green step referencing it in ac_refs, or set planLedger.blockOnUncoveredAcs=false`;
    return gateDecision(mode, reason) ?? ALLOW;
  }
  const hint = rawOr(() => alt(get(ledger, "plan_doc"), "<plan_doc>"), "<plan_doc>");
  return stepsDecision({ mode, ledger, cur, hint });
}

/** The native `plan-ledger` built-in PreToolUse module. */
export function planLedgerModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "plan-ledger",
    run: (event, ctx) => Promise.resolve(evaluate(event, ctx, options)),
  };
}
