/**
 * Executable documentation blocks (#363). A doc marks one fenced `bash` block
 * with `<!-- opencode-doc:<name>:start -->` and `<!-- opencode-doc:<name>:end -->`;
 * the live `docs.*` scenarios run it word for word in an isolated profile.
 *
 * Only two commands are redirected, through PATH shims: `npx @toolu/plugins`
 * runs the checkout's CLI bundle (the release under test is not on npm yet) and
 * `opencode` runs the pinned binary. `npx` with any other package fails, so a
 * doc can never reach an unpinned network package unnoticed. Everything else
 * is the reader's own shell.
 */
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { delimiter, join } from "node:path";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { cliBundle } from "./scenarios-cli.ts";
import { npmSpec, type EntryContext } from "./scenarios-entry.ts";
import { ContractError } from "./schema.ts";
import type { ProbeSession } from "./session.ts";

/** The `npx` shim's exit for any package other than `@toolu/plugins`. */
export const NPX_REFUSED_EXIT = 97;
const NPX_PACKAGE = "@toolu/plugins";
/** A hang guard: a block may start the host several times in a fresh profile. */
const DOC_TIMEOUT_MS = 900_000;
const FENCE = /^```bash\n([\s\S]*?)^```$/gm;

/** The one marker `marker` in `text`, or a ContractError naming the block. */
function markerAt(text: string, marker: string, name: string): number {
  const at = text.indexOf(marker);
  if (at < 0 || text.includes(marker, at + marker.length))
    throw new ContractError(`doc block ${name}: expected exactly one ${marker}`);
  return at;
}

/** The body of the single fenced `bash` block between `name`'s markers. */
export function docBlock(text: string, name: string): string {
  const start = `<!-- opencode-doc:${name}:start -->`;
  const end = `<!-- opencode-doc:${name}:end -->`;
  const from = markerAt(text, start, name) + start.length;
  const to = markerAt(text, end, name);
  if (to < from) throw new ContractError(`doc block ${name}: ${end} precedes ${start}`);
  const between = text.slice(from, to);
  const fences = [...between.matchAll(FENCE)];
  const [fence] = fences;
  if (fences.length !== 1 || fence === undefined || between.replace(fence[0], "").trim() !== "")
    throw new ContractError(
      `doc block ${name}: expected one fenced bash block between its markers`,
    );
  return fence[1] ?? "";
}

type ShimTargets = { node: string; cli: string; opencode: string };

/** `value` wrapped in one single-quoted sh word: nothing inside it expands. */
export function shQuote(value: string): string {
  return `'${value.replaceAll("'", `'\\''`)}'`;
}

/** `npx` and `opencode` shims in `dir`, for the front of a doc block's PATH. */
export function writeShims(dir: string, targets: ShimTargets): void {
  mkdirSync(dir, { recursive: true });
  const npx = [
    "#!/bin/sh",
    `if [ "$1" != ${shQuote(NPX_PACKAGE)} ]; then`,
    `  echo "doc shim: only ${NPX_PACKAGE} is redirected, got: $1" >&2`,
    `  exit ${NPX_REFUSED_EXIT}`,
    "fi",
    "shift",
    `exec ${shQuote(targets.node)} ${shQuote(targets.cli)} "$@"`,
    "",
  ].join("\n");
  const opencode = `#!/bin/sh\nexec ${shQuote(targets.opencode)} "$@"\n`;
  for (const [name, body] of Object.entries({ npx, opencode })) {
    writeFileSync(join(dir, name), body);
    chmodSync(join(dir, name), 0o755);
  }
}

/** `script` under `bash -euo pipefail` in `cwd`, with the shims first on PATH. */
export async function runScript(
  script: string,
  opts: { cwd: string; shims: string; env: Record<string, string> },
): Promise<RunResult> {
  const res = await run(["bash", "-euo", "pipefail", "-c", script], {
    cwd: opts.cwd,
    env: { ...opts.env, PATH: `${opts.shims}${delimiter}${process.env.PATH ?? ""}` },
    stdin: "",
    timeoutMs: DOC_TIMEOUT_MS,
  });
  if (res.timedOut)
    throw new ContractError(
      `doc block timed out after ${DOC_TIMEOUT_MS} ms; stderr: ${res.stderr.slice(-2000)}`,
    );
  return res;
}

/** Run a doc block in the session's project, the way a reader of the doc would. */
export async function runDocBlock(
  ctx: EntryContext,
  s: ProbeSession,
  script: string,
): Promise<RunResult> {
  const node = Bun.which("node");
  if (node === null) throw new ContractError("node is not on PATH; the CLI bundle targets Node");
  const shims = s.outside("doc-bin");
  writeShims(shims, { node, cli: await cliBundle(ctx), opencode: ctx.bin });
  return runScript(script, {
    cwd: s.sb.project,
    shims,
    env: { ...s.env, TOOLU_OPENCODE_PACKAGE: npmSpec(ctx.tarball) },
  });
}
