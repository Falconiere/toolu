// Deduplicating stderr reporter for the dashboard's polling paths.
//
// discoverProjects / syncSessions / the activity fingerprint all re-run on every
// poll tick (cfg.pollMs, default 1500ms). A plain console.error in one of their
// catch blocks turns a single persistent fault — one unreadable directory, one
// corrupt ledger — into ~40 identical lines a minute for as long as the server
// runs. Swallowing the error is not the answer either: the fault then vanishes
// entirely. warnOnce reports the FIRST occurrence of each distinct fault and
// stays quiet afterwards.
//
// Keyed by caller-supplied string (normally the path plus the operation), so two
// different failures on the same path still both surface. The key set is bounded
// by the number of distinct faulty paths, which is finite and small; a healthy
// server never adds an entry at all.

const reported = new Set<string>();

/** Report `message` on stderr the first time this `key` is seen; no-op after. */
export function warnOnce(key: string, message: string): void {
  if (reported.has(key)) return;
  reported.add(key);
  console.error(message);
}

/** Drop the memo. Tests only — a fresh process starts empty in production. */
export function resetWarnOnce(): void {
  reported.clear();
}
