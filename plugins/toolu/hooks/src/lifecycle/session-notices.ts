/**
 * Once-per-machine notices of toolu's SessionStart (#263). Each is shown
 * until its sentinel under `<config root>/toolu/` exists, and the sentinel is
 * written the first time it is shown. A failed write only repeats the notice.
 */
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { isJsonObject, type JsonObject } from "@toolu/core/config";
import type { HostName } from "@toolu/core/host";

/** `mkdir -p dir && : > file`, best effort. */
function touch(file: string): void {
  try {
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, "");
  } catch {
    // Parity with `2>/dev/null`: the notice simply shows again next session.
  }
}

function once(sentinel: string, text: string, show: boolean): string | undefined {
  if (!show || existsSync(sentinel)) return undefined;
  touch(sentinel);
  return text;
}

/** A preset, or any gate object carrying `mode`: the user already chose how gates deliver. */
function deliveryPinned(config: JsonObject): boolean {
  const gates = config.gates;
  if (!isJsonObject(gates)) return false;
  if (gates.preset !== undefined && gates.preset !== null) return true;
  return Object.values(gates).some((gate) => isJsonObject(gate) && Object.hasOwn(gate, "mode"));
}

export function gatePresetNotice(configRoot: string, config: JsonObject): string | undefined {
  return once(
    join(configRoot, "toolu", ".gate-preset-notice-v6"),
    'toolu gates no longer prompt: the `balanced` preset advises on push-review and denylist hits, and the quality gate still blocks only `git commit`/`git push`. Pin a prompt with `gates.<name>.mode: ask`, or the old hard denies with `{"gates":{"preset":"strict"}}`.',
    !deliveryPinned(config),
  );
}

export function deliveryFlowNotice(configRoot: string, host: HostName): string | undefined {
  const install =
    host === "codex"
      ? "Install with `npx @toolu/plugins install delivery-flow --host codex`, then invoke `$delivery-flow:delivery-flow`."
      : host === "opencode"
        ? 'Add `delivery-flow` to `enabled` in `.opencode/toolu/plugins.json`, then load `skill({ name: "delivery-flow-delivery-flow" })`.'
        : "Install with `/plugin install delivery-flow@toolu`, then invoke `/delivery-flow:delivery-flow`.";
  return once(
    join(configRoot, "toolu", ".delivery-flow-migration-v7"),
    `WARN: toolu workflow skills moved to delivery-flow (brainstorm, spec, spec-review, plan, plan-review, execution, test). ${install}`,
    true,
  );
}
