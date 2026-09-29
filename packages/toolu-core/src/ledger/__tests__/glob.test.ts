/**
 * `matchesAny` vs bash `case "$path" in $glob)` (#256): every (path, glob)
 * pair runs through real bash in one process, and the TypeScript matcher must
 * give the same answer for each. The pairs include the docs-sync defaults.
 */
import { expect, test } from "bun:test";
import { run } from "@toolu/conformance/harness/spawn";
import { DOCS_SYNC_DEFAULTS } from "../../config/docs-sync-config.ts";
import { matchesAny } from "../glob.ts";

const PATHS = [
  "README.md",
  "docs/config.md",
  "docs/releases/v1.md",
  "plugins/toolu/README.md",
  "plugins/x/skills/y/SKILL.md",
  "a/b/workflows/w.md",
  "src/a.ts",
  "lib/foo.sh",
  "plugins/x/commands/c.md",
  ".claude-plugin/plugin.json",
  "tsconfig.json",
  "knip.config.json",
  "dir with space/é.ts",
  "[x].md",
  "a\\b",
  "-dash.ts",
];

const GLOBS = [
  ...DOCS_SYNC_DEFAULTS.surfaces,
  ...DOCS_SYNC_DEFAULTS.surfaceExcludes,
  ...DOCS_SYNC_DEFAULTS.codeSurfaces,
  "?EADME.md",
  "[a-d]ocs/*",
  "[!d]*",
  "[^d]*",
  "*[[:space:]]*",
  "*[[:upper:]]*.md",
  "\\[x\\].md",
  "[x].md",
  "[",
  "a\\\\b",
  "*.[tj]s",
  "[]]*",
  "*/*/*",
  "-*",
  "",
];

test("matchesAny agrees with bash case patterns on every pair", async () => {
  const script = 'for g in "${@:2}"; do case "$1" in $g) printf 1;; *) printf 0;; esac; done';
  const results = await Promise.all(
    PATHS.map((path) =>
      run(["bash", "-c", `${script}`, "_", path, ...GLOBS], { env: { LC_ALL: "C.UTF-8" } }),
    ),
  );
  PATHS.forEach((path, i) => {
    const bash = results[i]?.stdout ?? "";
    const ts = GLOBS.map((glob) => (glob !== "" && matchesAny(path, [glob]) ? "1" : "0")).join("");
    const bashNonEmpty = [...bash].map((bit, j) => (GLOBS[j] === "" ? "0" : bit)).join("");
    expect({ path, ts }).toEqual({ path, ts: bashNonEmpty });
  });
});
