import { expect, test } from "bun:test";
import { readFileSync, readlinkSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { acceptancePackageDir, installShim } from "../scenarios-entry.ts";
import { openSession } from "../session.ts";

// Real sandboxes: the shim links the package the acceptance run names (#362 regression controls).

const Manifest = z.looseObject({ name: z.string() });

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
