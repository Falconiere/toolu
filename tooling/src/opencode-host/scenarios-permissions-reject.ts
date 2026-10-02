/** Real user rejection through the pinned host's permission reply API. */
import { z } from "zod";
import { withServe } from "./host-run.ts";
import { session, type PretoolScenario } from "./pretool-shared.ts";
import type { ScenarioContext } from "./scenario.ts";
import { ContractError } from "./schema.ts";
import type { ProbeSession } from "./session.ts";

const Session = z.object({ id: z.string() });
const Asked = z.object({ id: z.string(), sessionID: z.string() });

async function post(url: string, path: string, body: object): Promise<unknown> {
  const response = await fetch(`${url}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(180_000),
  });
  const text = await response.text();
  if (!response.ok) throw new ContractError(`${path}: HTTP ${response.status}: ${text}`);
  return text === "" ? null : JSON.parse(text);
}

function query(project: string): string {
  return `?directory=${encodeURIComponent(project)}`;
}

async function prompt(url: string, project: string, id: string): Promise<void> {
  await post(url, `/session/${id}/prompt_async${query(project)}`, {
    model: { providerID: "probe", modelID: "scripted" },
    parts: [{ type: "text", text: "PROBE:permissions.reject" }],
  });
}

async function asked(
  s: ProbeSession,
  seen: ReadonlySet<string>,
  deadline = Date.now() + 180_000,
): Promise<string> {
  const found = s
    .log()
    .filter((entry) => entry.type === "permission.asked")
    .map((entry) => Asked.safeParse(entry.permission))
    .find((parsed) => parsed.success && !seen.has(parsed.data.id));
  if (found?.success) return found.data.id;
  if (Date.now() >= deadline)
    throw new ContractError("native permission.asked did not arrive in the isolated session");
  await Bun.sleep(100);
  return asked(s, seen, deadline);
}

async function idle(
  s: ProbeSession,
  count: number,
  deadline = Date.now() + 180_000,
): Promise<void> {
  if (s.log().filter((entry) => entry.type === "session.idle").length > count) return;
  if (Date.now() >= deadline)
    throw new ContractError("session did not become idle after a rejected permission");
  await Bun.sleep(100);
  return idle(s, count, deadline);
}

async function rejectOnce(
  url: string,
  s: ProbeSession,
  sessionID: string,
  seen: Set<string>,
): Promise<void> {
  const idleBefore = s.log().filter((entry) => entry.type === "session.idle").length;
  await prompt(url, s.sb.project, sessionID);
  const permissionID = await asked(s, seen);
  seen.add(permissionID);
  await post(url, `/session/${sessionID}/permissions/${permissionID}${query(s.sb.project)}`, {
    response: "reject",
  });
  await idle(s, idleBefore);
}

async function nativeReject(ctx: ScenarioContext) {
  using s = session(ctx, {
    config: () => ({ permission: { bash: "ask" } }),
    scripts: {
      "permissions.reject": [
        { tool: "bash", args: { command: "touch rejected-marker", description: "reject" } },
      ],
    },
  });
  const observed = await withServe(ctx.bin, s, async (url) => {
    const created = Session.parse(await post(url, `/session${query(s.sb.project)}`, {}));
    const ids = new Set<string>();
    await rejectOnce(url, s, created.id, ids);
    await rejectOnce(url, s, created.id, ids);
    return { twoFreshPrompts: ids.size === 2, markerAbsent: !s.exists("rejected-marker") };
  });
  return { pass: Object.values(observed).every(Boolean), observed };
}

export const REJECTION_SCENARIOS: PretoolScenario[] = [
  { id: "permissions.reject", run: nativeReject },
];
