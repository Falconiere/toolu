/**
 * One enforcing toolu instance per OpenCode instance directory (#336).
 *
 * A project can list the npm package in `opencode.json` and also keep a local
 * `.opencode/plugins/` shim; the host loads both, often as two module copies.
 * The claim lives on `globalThis` under a registered symbol so every copy in the
 * process shares it. The admitted instance releases its directory in `dispose`,
 * which the host calls from the instance finalizer, so a re-created instance
 * enforces again.
 */
const CLAIMS = Symbol.for("toolu.opencode.instances");

function claims(): object {
  const existing: unknown = Reflect.get(globalThis, CLAIMS);
  if (typeof existing === "object" && existing !== null) return existing;
  // Keys are absolute instance directories, which never collide with Object.prototype names.
  const created = {};
  Reflect.set(globalThis, CLAIMS, created);
  return created;
}

/** True for the first claim of `directory`; false while another instance holds it. */
export function claimInstance(directory: string): boolean {
  const held = claims();
  if (Reflect.has(held, directory)) return false;
  Reflect.set(held, directory, true);
  return true;
}

export function releaseInstance(directory: string): void {
  Reflect.deleteProperty(claims(), directory);
}
