// Cross-plugin register.sh invariants.
//
// The ts-quality/rust-quality byte-parity drift guard already lives in
// plugins/rust-quality/hooks/__tests__/register-sync.bats -- this suite covers the
// invariant that applies to EVERY plugin's register.sh instead.
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../../..");

test.concurrent("every plugin register.sh namespaces its registry entries", () => {
  // The core dispatcher SKIPS any registry module not named
  // "<plugin-spec>__<name>.sh" (fail-closed, see hooks/lib/dispatch.sh). A
  // register.sh that forgets the namespace installs a module that never runs --
  // silently, since "skipped" and "no findings" look identical.
  const scripts = [...new Bun.Glob("plugins/*/hooks/register.sh").scanSync({ cwd: ROOT })]
    .toSorted()
    .map((rel) => join(ROOT, rel))
    .filter((file) => existsSync(file));
  for (const file of scripts) {
    const body = readFileSync(file, "utf8");
    expect({ file, hasSpec: body.includes("SPEC=") }).toEqual({ file, hasSpec: true });
    expect({ file, namespaced: body.includes("${SPEC}__") }).toEqual({ file, namespaced: true });
  }
  // Guard the guard: a bad glob would vacuously pass the loop above.
  expect(scripts.length).toBeGreaterThan(0);
});
