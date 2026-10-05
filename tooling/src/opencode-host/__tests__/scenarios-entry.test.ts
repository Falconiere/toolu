import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync, readlinkSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { stageOpencode } from "../../npm-pack.ts";
import { acceptancePackageDir, installShim, pinCheckoutCore, ROOT } from "../scenarios-entry.ts";
import { openSession } from "../session.ts";

// Real sandboxes: the shim links the package the acceptance run names (#362 regression controls).

const Manifest = z.looseObject({ name: z.string() });
const Versioned = z.looseObject({
  name: z.string(),
  version: z.string(),
  dependencies: z.record(z.string(), z.string()),
});

function manifest(path: string): z.infer<typeof Versioned> {
  return Versioned.parse(JSON.parse(readFileSync(path, "utf8")));
}

test.concurrent("without an override the shim package is the checkout's @toolu/opencode", () => {
  const dir = acceptancePackageDir({});
  expect(Manifest.parse(JSON.parse(readFileSync(join(dir, "package.json"), "utf8"))).name).toBe(
    "@toolu/opencode",
  );
  expect(acceptancePackageDir({ TOOLU_ACCEPTANCE_PACKAGE: "" })).toBe(dir);
});

test.concurrent("TOOLU_ACCEPTANCE_PACKAGE redirects the shim's node_modules link", () => {
  using cache = createSandbox();
  using staged = createSandbox();
  using session = openSession(cache.root);
  const packageDir = acceptancePackageDir({ TOOLU_ACCEPTANCE_PACKAGE: staged.root });
  expect(packageDir).toBe(staged.root);
  installShim(session, session.sb.project, packageDir);
  expect(readlinkSync(join(session.sb.project, "node_modules/@toolu/opencode"))).toBe(staged.root);
  expect(session.sb.read(".opencode/plugins/toolu.ts")).toBe(
    'export { default } from "@toolu/opencode";\n',
  );
});

// A release PR raises the @toolu/core floor before that core is published (#394):
// the npm-route tarball must install the checkout's core, never the registry's.
test.concurrent("the staged package depends on the checkout's packed @toolu/core", () => {
  using work = createSandbox();
  const stage = stageOpencode(work.root);
  const before = manifest(join(stage, "package.json"));
  const core = pinCheckoutCore(stage, work.root);
  const after = manifest(join(stage, "package.json"));
  expect(after.dependencies["@toolu/core"]).toBe(`file:${core}`);
  expect({ ...after, dependencies: {} }).toEqual({ ...before, dependencies: {} });
  expect(after.dependencies["zod"]).toBe(before.dependencies["zod"]);
  const packed = spawnSync("tar", ["-xzOf", core, "package/package.json"], { encoding: "utf8" });
  expect(packed.status).toBe(0);
  const checkout = manifest(join(ROOT, "packages/toolu-core/package.json"));
  expect(Versioned.parse(JSON.parse(packed.stdout))).toMatchObject({
    name: "@toolu/core",
    version: checkout.version,
  });
});
