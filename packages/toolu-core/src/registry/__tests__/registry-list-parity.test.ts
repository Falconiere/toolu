/**
 * `listRegistryDir` vs the real `toolu_dispatch_modules` (#257, AC-1). Every
 * fixture module appends its own name to a log when bash runs it, so the log
 * is bash's dispatch order, and its "lacks namespace" warnings are its
 * rejections. Bash runs under `LC_ALL=C`, whose glob order is byte order.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { listRegistryDir } from "../registry-list.ts";

const LIB = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib");

const MODULES = [
  "a@t__one.sh",
  "B@t__two.sh",
  "b@t__three.sh",
  "_x@t__under.sh",
  "1@t__digit.sh",
  "a.b@t__dot.sh",
  "a-b@t__dash.sh",
  "a@t__One.sh",
  "a@t__.sh",
  "a@t__x__y.sh",
  "__nospec.sh",
  "nosep.sh",
  "a b@t__space.sh",
  ".hidden@t__h.sh",
];

test.concurrent("orders and rejects registry modules exactly as dispatch.sh does", async () => {
  using sb = createSandbox();
  const dir = sb.path("registry/post-tools.d");
  const log = sb.path("dispatch.log");
  mkdirSync(dir, { recursive: true });
  const body = `printf '%s\\n' "\${0##*/}" >> "${log}"\n`;
  for (const name of MODULES) writeFileSync(join(dir, name), body);
  writeFileSync(sb.path("target.sh"), body);
  symlinkSync(sb.path("target.sh"), join(dir, "c@t__link.sh"));
  symlinkSync(sb.path("missing.sh"), join(dir, "c@t__dangling.sh"));
  mkdirSync(join(dir, "d@t__dir.sh"));
  writeFileSync(join(dir, "a@t__x.sh.tmp.123"), body);
  writeFileSync(join(dir, "e@t__esm.js"), "export default {};\n");
  writeFileSync(join(dir, "notes.txt"), "");

  const script =
    '. "$1/detect.sh"; . "$1/dispatch.sh"; input="{}"; export input; toolu_dispatch_modules "$2/none" PostToolUse "$3"';
  const res = await run(["bash", "-c", script, "_", LIB, sb.root, dir], {
    env: { HOME: sb.home, LC_ALL: "C" },
  });
  expect(res.exitCode).toBe(0);
  const bashOrder = readFileSync(log, "utf8").trim().split("\n");
  const bashRejected = [...res.stderr.matchAll(/registry module (.+) lacks/gu)].map(
    (m) => m[1] ?? "",
  );

  const listing = listRegistryDir(dir);
  const tsBash = listing.entries.filter((e) => e.kind === "bash");
  // A symlink runs under its link name; bash's $0 is the link path too.
  expect(tsBash.map((e) => e.file)).toEqual(bashOrder);
  expect(listing.rejected).toEqual(bashRejected);
  expect(listing.entries.filter((e) => e.kind === "esm").map((e) => e.file)).toEqual([
    "e@t__esm.js",
  ]);
  expect(tsBash.find((e) => e.file === "a@t__x__y.sh")).toMatchObject({
    spec: "a@t",
    name: "x__y",
  });
});

test.concurrent("an absent directory lists nothing", () => {
  using sb = createSandbox();
  expect(listRegistryDir(sb.path("nope"))).toEqual({ entries: [], rejected: [] });
});
