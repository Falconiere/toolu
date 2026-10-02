/**
 * Shared shape and observation helpers for the live host scenarios (#335).
 * A scenario returns a verdict for its claim plus deterministic observations.
 * When its precondition fails (the host never reached the probed point), it
 * throws instead: a broken harness must never be recorded as `unsupported`.
 */
import { z } from "zod";
import type { RecordedRequest } from "./provider.ts";
import { ContractError, type ProbeResult, type Verdict } from "./schema.ts";
import type { ProbeSession } from "./session.ts";

type Observed = Record<string, boolean | string | number>;
export type Observation = { verdict: Verdict; observed: Observed };
export type ScenarioContext = { bin: string; cacheRoot: string };
export type Scenario = Omit<ProbeResult, "verdict" | "observed"> & {
  run: (ctx: ScenarioContext) => Promise<Observation>;
};

export function verdict(claimHolds: boolean, observed: Observed): Observation {
  return { verdict: claimHolds ? "supported" : "unsupported", observed };
}

/** Throw when the host never reached what the scenario probes. */
export function precondition(id: string, holds: boolean, what: string): void {
  if (!holds) throw new ContractError(`scenario ${id} invalid: ${what}`);
}

export function entries(session: ProbeSession, kind: string): Array<Record<string, unknown>> {
  return session.log().filter((entry) => entry.kind === kind);
}

export function eventTypes(session: ProbeSession): string[] {
  return entries(session, "event").flatMap((entry) =>
    typeof entry.type === "string" ? [entry.type] : [],
  );
}

export function hookedTools(session: ProbeSession, kind: "before" | "after"): string[] {
  return entries(session, kind).flatMap((entry) =>
    typeof entry.tool === "string" ? [entry.tool] : [],
  );
}

const Message = z.looseObject({ role: z.string(), content: z.unknown() });
const ChatBody = z.looseObject({
  messages: z.array(Message),
  tools: z.array(z.unknown()).optional(),
});
const NamedTool = z.looseObject({ function: z.looseObject({ name: z.string() }) });

function chatBodies(requests: RecordedRequest[]): Array<z.infer<typeof ChatBody>> {
  return requests.flatMap((req) => {
    const parsed = ChatBody.safeParse(req.body);
    return parsed.success ? [parsed.data] : [];
  });
}

/** Text of every message with `role` across all recorded chat requests. */
export function messagesText(session: ProbeSession, role: string): string {
  return chatBodies(session.requests())
    .flatMap((body) =>
      body.messages.filter((m) => m.role === role).map((m) => JSON.stringify(m.content)),
    )
    .join("\n");
}

/** Messages with `role` in the last chat request; each earlier tool result appears there exactly once. */
export function finalMessages(session: ProbeSession, role: string): string[] {
  const last = chatBodies(session.requests()).at(-1);
  return (last?.messages ?? [])
    .filter((m) => m.role === role)
    .map((m) => JSON.stringify(m.content));
}

/** Tool names offered to the model in any request that offered tools. */
export function offeredTools(session: ProbeSession): string[] {
  const names = chatBodies(session.requests()).flatMap((body) =>
    (body.tools ?? []).flatMap((tool) => {
      const parsed = NamedTool.safeParse(tool);
      return parsed.success ? [parsed.data.function.name] : [];
    }),
  );
  return [...new Set(names)].toSorted();
}

export function toolRequestCount(session: ProbeSession): number {
  return chatBodies(session.requests()).filter((body) => (body.tools ?? []).length > 0).length;
}

/** Everything the model was sent, joined into one searchable string. */
export function allRequestText(session: ProbeSession): string {
  return session
    .requests()
    .map((req) => JSON.stringify(req.body))
    .join("\n");
}
