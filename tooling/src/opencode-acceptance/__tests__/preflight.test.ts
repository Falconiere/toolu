import { expect, test } from "bun:test";
import { chmodSync, symlinkSync } from "node:fs";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { preflight } from "../preflight.ts";

// Real binaries on a PATH built for the test (#362 AC-3): a missing or broken tool is named, never skipped.

function linkTool(dir: string, name: string): void {
  const real = Bun.which(name);
  if (real === null) throw new Error(`${name} must exist on this machine for the test`);
  symlinkSync(real, `${dir}/${name}`);
}

test.concurrent("tools absent from PATH are reported missing, present ones with their version", async () => {
  using sb = createSandbox();
  const bin = sb.path("bin");
  sb.write("bin/.keep", "");
  linkTool(bin, "git");
  linkTool(bin, "tar");
  const report = await preflight({ PATH: bin });
  expect(report.missing).toEqual(["ast-grep (not on PATH)", "npm (not on PATH)"]);
  expect(report.tools.git).toMatch(/^git version \d/);
  expect(Object.keys(report.tools).toSorted()).toEqual(["git", "tar"]);
});

test.concurrent("a tool whose --version fails is reported broken", async () => {
  using sb = createSandbox();
  const bin = sb.path("bin");
  sb.write("bin/ast-grep", "#!/bin/sh\necho broken >&2\nexit 3\n");
  chmodSync(sb.path("bin/ast-grep"), 0o755);
  linkTool(bin, "git");
  const report = await preflight({ PATH: `${bin}:/bin:/usr/bin` });
  expect(report.missing).toContain("ast-grep (--version failed)");
});
