/**
 * Whether each gate reaches a file, read from that gate's own committed
 * config: the root tsconfig, the `format:check` script or the xtask fmt step
 * when that script is gone, every .oxlintrc.json with its lint targets (the
 * same discovery `lint:ts` runs), .jscpd.json and knip.json. An exact-path
 * ignore entry is a legacy exemption and leaves the file reached.
 */
import { readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { lintDirs } from "../lint-ts.ts";
import { covers, isExactPath, jscpdIgnore, matches, plainGlobs } from "./reach-files.ts";
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
  plainGlobs("tsconfig.json", [...include, ...exclude]);
  return (file) =>
    include.some((glob) => covers(glob, file)) && !exclude.some((glob) => covers(glob, file));
}

const FMT_GATE = "crates/xtask/src/gate.rs";
const FMT_STEP = '"fmt" => cargo(root, &["fmt", "--all", "--check"]),';

/** Format left the TypeScript script. The replacement is the xtask fmt step. */
function requireFmtReplacement(root: string): void {
  let text: string;
  try {
    text = readFileSync(join(root, FMT_GATE), "utf8");
  } catch (err: unknown) {
    const detail = err instanceof Error ? err.message : String(err);
    fatal(`${FMT_GATE}: unreadable (${detail})`);
  }
  if (!text.includes(FMT_STEP)) {
    fatal(
      `package.json: format:check is absent; ${FMT_GATE} must run \`cargo fmt --all --check\``,
    );
  }
}

function format(root: string): Reached | undefined {
  const scripts = readJson(root, "package.json", PackageJsonSchema).scripts;
  if (!Object.hasOwn(scripts, "format:check")) {
    requireFmtReplacement(root);
    return undefined;
  }
  const script = scripts["format:check"] ?? "";
  const words = script.split(/\s+/).filter(Boolean);
  const flag = words.indexOf("--check");
  if (words[0] !== "oxfmt" || flag === -1) {
    fatal('package.json: format:check is not an "oxfmt --check <paths>" script');
  }
  const paths = plainGlobs("package.json format:check", words.slice(flag + 1));
  return (file) => paths.some((path) => covers(path, file));
}

/**
 * oxlint ignorePatterns are gitignore-style: a leading `/` anchors the pattern
 * to the config directory, a trailing `/` names a directory, and anything else
 * matches at any depth. Over-matching can only make a file count unreached.
 */
function ignoredBy(pattern: string, path: string): boolean {
  const name = pattern.replace(/^\//, "").replace(/\/$/, "");
  const globs = pattern.startsWith("/")
    ? [name, `${name}/**`]
    : [name, `${name}/**`, `**/${name}`, `**/${name}/**`];
  return globs.some((glob) => matches(glob, path));
}

function oxlint(root: string): Reached {
  const dirs = lintDirs(root).map(({ dir, targets }) => {
    const rel = relative(root, dir);
    const config = `${rel}/.oxlintrc.json`;
    const { ignorePatterns } = readJson(root, config, OxlintSchema);
    return { rel, targets, ignorePatterns: plainGlobs(config, ignorePatterns) };
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
  const { structural } = jscpdIgnore(ignore);
  plainGlobs(".jscpd.json", [...path, ...structural]);
  return (file) =>
    path.some((entry) => covers(entry, file)) && !structural.some((glob) => matches(glob, file));
}

function knip(root: string): Reached {
  const { workspaces } = readJson(root, "knip.json", KnipSchema);
  for (const { project, ignore } of Object.values(workspaces)) {
    plainGlobs("knip.json", [...project, ...ignore]);
  }
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
  const formatReach = format(root);
  const tools: Array<readonly [Tool, Reached]> = [["typecheck", typecheck(root)]];
  if (formatReach !== undefined) tools.push(["format", formatReach]);
  tools.push(["oxlint", oxlint(root)], ["jscpd", jscpd(root)], ["knip", knip(root)]);
  return tools;
}
