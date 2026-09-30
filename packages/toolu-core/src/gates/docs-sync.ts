/**
 * docs-sync (#262): on a `git push` whose branch diff changes code but no
 * documentation surface, nudge (or, per `docsSync` mode, ask or deny). An
 * attestation keyed to the diff sha says no doc change is needed and silences
 * it. A port of `pre-tools/modules/docs-sync.sh`, reading the pushed
 * repository (#283 item 9). `off` is indistinguishable from the gate not
 * existing: no output, no telemetry.
 */
import { gateDecision, gateMode, type GateMode } from "../config/gate-mode.ts";
import {
  docsSyncCodeSurfaces,
  docsSyncSurfaceExcludes,
  docsSyncSurfaces,
} from "../config/docs-sync-config.ts";
import type { Decision } from "../decision/decision.ts";
import { baseBranch, branchSlug } from "../detect/detect-branch.ts";
import { envValue } from "../host/host-name.ts";
import { projectStateDir } from "../host/host-roots.ts";
import { matchesAny } from "../ledger/glob.ts";
import { alt, get } from "../ledger/ledger-jq.ts";
import { isFile } from "../ledger/ledger-parse.ts";
import { rawOr, readJson } from "../ledger/verdict-gates.ts";
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
import { gitAt, pushTarget, refExists } from "./push-target.ts";

/** The closing sentence: what happens next depends on the mode. */
const CONSEQUENCE: Readonly<Record<Exclude<GateMode, "off">, string>> = {
  block: "Push denied until a doc is updated or a valid attestation is written.",
  ask: "Approve to push anyway, or update the doc first.",
  advise: "Advisory only — this does not block the push.",
};

function evaluate(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  const target = pushTarget(event, ctx);
  if (target === undefined) return ALLOW;
  const { root, branch } = target;
  if (branch === "" || branch === "HEAD") return ALLOW;
  const base = envValue(ctx.env, "DOCS_SYNC_BASE") ?? baseBranch(root, ctx.env);
  if (!refExists(root, base, ctx.env) || branch === base) return ALLOW;
  const changed = (gitAt(root, ["diff", "--name-only", `${base}...HEAD`], ctx.env) ?? "")
    .split("\n")
    .filter((path) => path !== "");
  if (changed.length === 0) return ALLOW;
  const sha = diffSha(root, base, { env: ctx.env }) ?? "";
  const config = gateConfig(ctx, options);
  const surfaces = docsSyncSurfaces(config);
  const excludes = docsSyncSurfaceExcludes(config);
  const code = docsSyncCodeSurfaces(config);
  const hasDoc = changed.some((p) => matchesAny(p, surfaces) && !matchesAny(p, excludes));
  const hasCode = changed.some((p) => matchesAny(p, code));
  if (!hasCode || hasDoc) return ALLOW;
  const mode = gateMode(config, "docsSync", { host: ctx.host, event: preToolHostEvent(event) });
  if (mode === "off") return ALLOW;
  const host = { env: ctx.env, host: ctx.host };
  const dir =
    envValue(ctx.env, "DOCS_SYNC_STATE_DIR") ?? projectStateDir("docs-sync", { ...host, root });
  const file = `${dir ?? ""}/${branchSlug(branch)}.json`;
  if (isFile(file) && sha !== "") {
    const doc = readJson(file) ?? null;
    if (rawOr(() => alt(get(doc, "diff_sha"), "")) === sha) {
      const decision = rawOr(() => alt(get(doc, "decision"), ""));
      telemetryAppend(root, "docs_attested", { decision }, host);
      return ALLOW;
    }
  }
  telemetryAppend(root, "docs_nudge", {}, host);
  const reason = `docs-sync: this branch changes code but no documentation surface (README / docs/*.md / SKILL.md). Update the doc that describes this behavior, OR attest none is needed by writing ${file} with { "version": 1, "diff_sha": "${sha}", "decision": "not-needed", "note": "why" }. ${CONSEQUENCE[mode]}`;
  return gateDecision(mode, reason) ?? ALLOW;
}

/** The native `docs-sync` built-in PreToolUse module. */
export function docsSyncModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "docs-sync",
    run: (event, ctx) => Promise.resolve(evaluate(event, ctx, options)),
  };
}
