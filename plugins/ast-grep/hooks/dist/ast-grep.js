#!/usr/bin/env bun
// @bun

// plugins/ast-grep/hooks/src/ast-grep.ts
import { spawnSync } from "child_process";
import { existsSync, statSync } from "fs";
var USAGE = `Usage: ast-grep.js <subcommand> [args...]

Subcommands:
  search <pattern> [--lang <L>] [flags]  Pattern search (--color never)
  files <pattern> [--lang <L>] [flags]   File paths only (--files-with-matches)
  scan <yaml> [flags]                    Rule-based scan (--report-style short --max-results 50)
  debug <pattern> [--lang <L>]           Debug pattern AST (--debug-query=pattern)

--lang is required for search/files/debug, but is auto-inferred from the
first path argument's extension if not supplied.

Pass-through flags: --globs <pat>, -A/-B/-C <N>, --max-results N
`;
var LANGS = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  rs: "rust",
  py: "python",
  go: "go",
  rb: "ruby",
  java: "java"
};
function isFile(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}
function inferredLang(args) {
  const hasLang = args.some((arg) => arg === "--lang" || arg === "-l" || arg.startsWith("--lang=") || arg.startsWith("-l="));
  const firstPath = args.find((arg) => existsSync(arg));
  if (hasLang || firstPath === undefined || !isFile(firstPath))
    return [];
  const ext = firstPath.slice(firstPath.lastIndexOf(".") + 1);
  const lang = Object.hasOwn(LANGS, ext) ? LANGS[ext] : undefined;
  return lang === undefined ? [] : ["--lang", lang];
}
function astGrepArgs(subcommand, args) {
  const [first = "", ...rest] = args;
  switch (subcommand) {
    case "search":
    case "files":
    case "debug": {
      if (first === "")
        return { error: `${subcommand} requires a pattern` };
      const mode = {
        search: [],
        files: ["--files-with-matches"],
        debug: ["--debug-query=pattern"]
      }[subcommand];
      return [
        "run",
        "--pattern",
        first,
        ...mode,
        "--color",
        "never",
        ...inferredLang(rest),
        ...rest
      ];
    }
    case "scan": {
      if (first === "")
        return { error: "scan requires inline YAML or rule file path" };
      const rules = isFile(first) ? ["--rule", first] : ["--inline-rules", first];
      return [
        "scan",
        ...rules,
        "--report-style",
        "short",
        "--max-results",
        "50",
        "--color",
        "never",
        ...rest
      ];
    }
    default:
      return { error: "" };
  }
}
function main(argv) {
  const binary = Bun.which("sg") ?? Bun.which("ast-grep");
  if (binary === null)
    return 0;
  const [subcommand = "", ...args] = argv;
  const built = astGrepArgs(subcommand, args);
  if (!Array.isArray(built)) {
    if (built.error === "")
      process.stdout.write(USAGE);
    else
      process.stderr.write(`ast-grep.js: ${built.error}
`);
    return 1;
  }
  const res = spawnSync(binary, built, { stdio: "inherit" });
  if (res.error !== undefined) {
    process.stderr.write(`ast-grep.js: ${res.error.message}
`);
    return 127;
  }
  return res.status ?? 1;
}
process.exitCode = main(process.argv.slice(2));
