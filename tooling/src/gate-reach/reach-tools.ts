/**
 * Whether each gate reaches a file, read from that gate's own committed
 * config: the root tsconfig, the `format:check` script, every .oxlintrc.json
 * with its lint targets (the same discovery `lint:ts` runs), .jscpd.json and
 * knip.json. An exact-path ignore entry is a legacy exemption and leaves the
 * file reached.
 */
import { relative } from "node:path";
import { lintDirs } from "../lint-ts.ts";
import { covers, isExactPath, matches } from "./reach-files.ts";
import {
  JscpdSchema,
  KnipSchema,
  OxlintSchema,
  PackageJsonSchema,
  TsConfigSchema,
  fatal,
  readJson,
} from "./reach-schema.ts";
import type { Tool } from "./reach-schema.ts";

type Reached = (file: string) => boolean;

function typecheck(root: string): Reached {
  const { include, exclude } = readJson(root, "tsconfig.json", TsConfigSchema);
  return (file) =>
    include.some((glob) => covers(glob, file)) && !exclude.some((glob) => covers(glob, file));
}

function format(root: string): Reached {
  const script = readJson(root, "package.json", PackageJsonSchema).scripts["format:check"] ?? "";
  const words = script.split(/\s+/).filter(Boolean);
  const flag = words.indexOf("--check");
  if (words[0] !== "oxfmt" || flag === -1) {
    fatal('package.json: format:check is not an "oxfmt --check <paths>" script');
  }
  const paths = words.slice(flag + 1);
  return (file) => paths.some((path) => covers(path, file));
}

/** oxlint ignorePatterns are gitignore-style: a bare name matches at any depth. */
function ignoredBy(pattern: string, path: string): boolean {
  return [pattern, `${pattern}/**`, `**/${pattern}`, `**/${pattern}/**`].some((glob) =>
    matches(glob, path),
  );
}

function oxlint(root: string): Reached {
  const dirs = lintDirs(root).map(({ dir, targets }) => {
    const rel = relative(root, dir);
    const { ignorePatterns } = readJson(root, `${rel}/.oxlintrc.json`, OxlintSchema);
    return { rel, targets, ignorePatterns };
  });
  return (file) =>
    dirs.some(({ rel, targets, ignorePatterns }) => {
      if (!targets.some((target) => file.startsWith(`${rel}/${target}/`))) return false;
      const inner = file.slice(rel.length + 1);
      return !ignorePatterns.some((pattern) => ignoredBy(pattern, inner));
    });
}

function jscpd(root: string): Reached {
  const { path, ignore } = readJson(root, ".jscpd.json", JscpdSchema);
  const structural = ignore.filter((glob) => !isExactPath(glob));
  return (file) =>
    path.some((entry) => covers(entry, file)) && !structural.some((glob) => matches(glob, file));
}

function knip(root: string): Reached {
  const { workspaces } = readJson(root, "knip.json", KnipSchema);
  // Longest first: a file belongs to its nearest workspace, "." takes the rest.
  const owners = Object.entries(workspaces).toSorted(([a], [b]) => b.length - a.length);
  return (file) => {
    const owner = owners.find(([dir]) => dir === "." || file.startsWith(`${dir}/`));
    if (owner === undefined) return false;
    const [dir, { project, ignore }] = owner;
    const inner = dir === "." ? file : file.slice(dir.length + 1);
    return (
      project.some((glob) => matches(glob.replace(/!$/, ""), inner)) &&
      !ignore.some((glob) => !isExactPath(glob) && matches(glob, inner))
    );
  };
}

/** One reach test per tool, in report order. */
export function loadReach(root: string): ReadonlyArray<readonly [Tool, Reached]> {
  return [
    ["typecheck", typecheck(root)],
    ["format", format(root)],
    ["oxlint", oxlint(root)],
    ["jscpd", jscpd(root)],
    ["knip", knip(root)],
  ];
}
