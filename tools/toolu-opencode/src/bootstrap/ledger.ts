/**
 * Startup ownership (#342). The ledger records the helper symlinks each plugin's
 * startup published, so a later startup can take them back when the plugin is
 * no longer selected, or no longer publishes one. Something toolu does not own
 * is never removed:
 * - a helper goes only while it is still the recorded symlink (a user file or a
 *   relinked path at that place is the user's);
 * - a registry module goes only when it is a regular file under a toolu
 *   plugin's own `<name>@toolu__` prefix (other specs, links and directories stay).
 * The ledger sits in the project, so it is not trusted: plugin names must be
 * catalog-shaped, specs are derived from them, and nothing whose real path
 * leaves the data root is touched.
 */
import { randomUUID } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, sep } from "node:path";
import { REGISTRY_DIRS } from "@toolu/core/registry";
import { z } from "zod";
import { opencodeRegistryRoot } from "../host/roots.ts";
import type { PluginManifest } from "../inventory/types.ts";
import type { OwnedHelper } from "./records.ts";

const HelperSchema = z.strictObject({ path: z.string().min(1), source: z.string().min(1) });

/** A catalog plugin name, the same shape the launcher accepts. */
const PluginName = z.string().regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/u);

const LedgerSchema = z.strictObject({
  version: z.literal(1),
  plugins: z.record(PluginName, z.strictObject({ helpers: z.array(HelperSchema) })),
});

export type Ledger = z.infer<typeof LedgerSchema>;
export type LedgerEntry = Ledger["plugins"][string];

/** What a cleanup did: what it took back and noted, and what it could not do. */
export type Cleanup = { failures: string[]; diagnostics: string[] };

function ledgerPath(dataRoot: string): string {
  return join(opencodeRegistryRoot(dataRoot), "startup-ledger.json");
}

export function emptyLedger(): Ledger {
  return { version: 1, plugins: {} };
}

/** The recorded ownership; an absent ledger is empty, an unreadable one empty with a diagnostic. */
export function readLedger(dataRoot: string): { ledger: Ledger; diagnostic?: string } {
  const path = ledgerPath(dataRoot);
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return { ledger: emptyLedger() };
    }
    return { ledger: emptyLedger(), diagnostic: `startup ledger ${path} unreadable; ignored` };
  }
  const parsed = LedgerSchema.safeParse(raw);
  if (parsed.success) return { ledger: parsed.data };
  return { ledger: emptyLedger(), diagnostic: `startup ledger ${path} invalid; ignored` };
}

/** The ledger's bytes: plugins in name order, so an unchanged startup writes nothing new. */
function serialize(ledger: Ledger): string {
  const names = Object.keys(ledger.plugins).toSorted();
  const plugins = Object.fromEntries(names.map((name) => [name, ledger.plugins[name]]));
  return `${JSON.stringify({ version: ledger.version, plugins }, null, 2)}\n`;
}

function currentBytes(path: string): string | undefined {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}

/**
 * Atomic: a unique temp file renamed over the ledger, skipped when the bytes
 * are already there. Returns why it failed, if it did.
 */
export function writeLedger(dataRoot: string, ledger: Ledger): string | undefined {
  const path = ledgerPath(dataRoot);
  const bytes = serialize(ledger);
  if (currentBytes(path) === bytes) return undefined;
  const tmp = `${path}.${randomUUID()}.tmp`;
  try {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(tmp, bytes, { flag: "wx" });
    renameSync(tmp, path);
    return undefined;
  } catch (error) {
    rmSync(tmp, { force: true });
    return `cannot write startup ledger ${path}: ${String(error)}`;
  }
}

function realpathOrUndefined(path: string): string | undefined {
  try {
    return realpathSync(path);
  } catch {
    return undefined;
  }
}

/** `dir` resolved, when it really is the data root or inside it. */
function realDirInDataRoot(dir: string, dataRoot: string): string | undefined {
  const real = realpathOrUndefined(dir);
  const root = realpathOrUndefined(dataRoot);
  if (real === undefined || root === undefined) return undefined;
  return real === root || real.startsWith(root + sep) ? real : undefined;
}

/** `path` with its directory resolved, when that directory really is inside the data root. */
function underDataRoot(path: string, dataRoot: string): string | undefined {
  const parent = realDirInDataRoot(dirname(path), dataRoot);
  return parent === undefined ? undefined : join(parent, basename(path));
}

function errorCode(error: unknown): string | undefined {
  return error instanceof Error && "code" in error && typeof error.code === "string"
    ? error.code
    : undefined;
}

/**
 * Whether `path` is still the symlink to `source` toolu published. A path that
 * cannot exist (ENOENT, or ENOTDIR from a crafted ledger) is gone; any other
 * error is reported rather than guessed.
 */
function stillOwned(path: string, source: string): "owned" | "other" | "gone" | { error: string } {
  try {
    const stat = lstatSync(path, { throwIfNoEntry: false });
    if (stat === undefined) return "gone";
    return stat.isSymbolicLink() && readlinkSync(path) === source ? "owned" : "other";
  } catch (error) {
    const code = errorCode(error);
    return code === "ENOENT" || code === "ENOTDIR" ? "gone" : { error: String(error) };
  }
}

/** Remove `helper` while it is still the symlink toolu published; anything else there is the user's. */
function retireHelper(owner: string, helper: OwnedHelper, dataRoot: string, out: Cleanup): boolean {
  if (realpathOrUndefined(dirname(helper.path)) === undefined) return true;
  const path = underDataRoot(helper.path, dataRoot);
  if (path === undefined) {
    out.diagnostics.push(`${owner}: ignored ledger path ${helper.path} outside the data root`);
    return true;
  }
  const owned = stillOwned(path, helper.source);
  if (owned === "gone") return true;
  if (owned === "other") {
    out.diagnostics.push(`${owner}: kept ${helper.path}, no longer toolu's`);
    return true;
  }
  if (owned !== "owned") {
    out.failures.push(`${owner}: cannot inspect helper ${helper.path}: ${owned.error}`);
    return false;
  }
  try {
    rmSync(path);
    out.diagnostics.push(`${owner}: removed helper ${helper.path}`);
    return true;
  } catch (error) {
    out.failures.push(`${owner}: cannot remove helper ${helper.path}: ${String(error)}`);
    return false;
  }
}

/** Retire `helpers`, returning the ones that could not be removed and stay owned. */
export function retireHelpers(
  owner: string,
  helpers: readonly OwnedHelper[],
  dataRoot: string,
  out: Cleanup,
): OwnedHelper[] {
  return helpers.filter((helper) => !retireHelper(owner, helper, dataRoot, out));
}

function isRegularFile(path: string): boolean {
  try {
    return lstatSync(path, { throwIfNoEntry: false })?.isFile() ?? false;
  } catch {
    return false;
  }
}

function moduleFiles(dir: string, spec: string): string[] {
  try {
    return readdirSync(dir)
      .filter((name) => name.startsWith(`${spec}__`) && /\.(?:js|sh)$/u.test(name))
      .map((name) => join(dir, name));
  } catch {
    return [];
  }
}

/** Remove the registry modules of `name@toolu`, a plugin that is not selected. */
function pruneModules(name: string, dataRoot: string, out: Cleanup): void {
  for (const dir of REGISTRY_DIRS) {
    const real = realDirInDataRoot(join(opencodeRegistryRoot(dataRoot), dir), dataRoot);
    if (real === undefined) continue;
    for (const path of moduleFiles(real, `${name}@toolu`)) {
      if (!isRegularFile(path)) continue;
      try {
        rmSync(path);
        out.diagnostics.push(`${name}: removed module ${path}`);
      } catch (error) {
        out.failures.push(`${name}: cannot remove module ${path}: ${String(error)}`);
      }
    }
  }
}

export type PruneInput = {
  dataRoot: string;
  selected: ReadonlySet<string>;
  catalog: readonly PluginManifest[];
  ledger: Ledger;
};

/**
 * Take back every contribution of a toolu plugin that is not selected: its
 * registry modules and its recorded helpers. Returns the ledger entries that
 * must stay because something could not be removed.
 */
export function pruneUnselected(input: PruneInput, out: Cleanup): Ledger["plugins"] {
  const names = new Set([
    ...Object.keys(input.ledger.plugins),
    ...input.catalog.map((plugin) => plugin.name),
  ]);
  const retained: Ledger["plugins"] = {};
  for (const name of [...names].toSorted()) {
    if (input.selected.has(name)) continue;
    pruneModules(name, input.dataRoot, out);
    const helpers = input.ledger.plugins[name]?.helpers ?? [];
    const left = retireHelpers(name, helpers, input.dataRoot, out);
    if (left.length > 0) retained[name] = { helpers: left };
  }
  return retained;
}
