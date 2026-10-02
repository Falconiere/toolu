/**
 * Which session a `session.deleted` event names, and the abort signal for that
 * session (#341). The pinned host delivers `properties.info.id`. `sessionID`
 * on `properties` or `data` is the other shape published in the same SDK pin.
 */

function nonEmpty(value: unknown): string | undefined {
  return typeof value === "string" && value !== "" ? value : undefined;
}

/** `properties.info.id` first, then `properties.sessionID`. */
function propertiesSessionId(bag: unknown): string | undefined {
  if (bag === null || typeof bag !== "object") return undefined;
  if ("info" in bag && bag.info !== null && typeof bag.info === "object" && "id" in bag.info) {
    const id = nonEmpty(bag.info.id);
    if (id !== undefined) return id;
  }
  return "sessionID" in bag ? nonEmpty(bag.sessionID) : undefined;
}

export function deletedSessionId(event: { type: string }): string | undefined {
  if (event.type !== "session.deleted") return undefined;
  if ("properties" in event) {
    const id = propertiesSessionId(event.properties);
    if (id !== undefined) return id;
  }
  if (!("data" in event) || event.data === null || typeof event.data !== "object") return undefined;
  return "sessionID" in event.data ? nonEmpty(event.data.sessionID) : undefined;
}

function dropClaims(claimed: Set<string>, sessionID: string): void {
  const prefix = `${sessionID}\0`;
  const drop: string[] = [];
  for (const key of claimed) {
    if (key.startsWith(prefix)) drop.push(key);
  }
  for (const key of drop) claimed.delete(key);
}

export type SessionSignals = {
  signalFor: (sessionID: string) => AbortSignal;
  dropSession: (sessionID: string, claimed: Set<string>) => void;
  clear: () => void;
};

/** One abort controller per session, all tied to `parent` so dispose stops the rest. */
export function sessionSignals(parent: AbortController): SessionSignals {
  const sessions = new Map<string, AbortController>();
  return {
    signalFor(sessionID) {
      const existing = sessions.get(sessionID);
      if (existing !== undefined) return existing.signal;
      const controller = new AbortController();
      if (parent.signal.aborted) controller.abort();
      else parent.signal.addEventListener("abort", () => controller.abort(), { once: true });
      sessions.set(sessionID, controller);
      return controller.signal;
    },
    dropSession(sessionID, claimed) {
      sessions.get(sessionID)?.abort();
      sessions.delete(sessionID);
      dropClaims(claimed, sessionID);
    },
    clear() {
      sessions.clear();
    },
  };
}
