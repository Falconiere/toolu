/**
 * Startup ownership (#342). The ledger records the helper symlinks each plugin's
 * startup published, so a later startup can take them back when the plugin is
 * no longer selected, or no longer publishes one. Something toolu does not own
 * is never removed:
 * - a helper goes only while it is still the recorded symlink (a user file or a
 *   relinked path at that place is the user's);
 * - a registry module goes only when it is a regular file under a toolu
 *   plugin's own `<name>@toolu__` prefix (other specs, links and directories stay).
 */
import { randomUUID } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { REGISTRY_DIRS } from "@toolu/core/registry";
import { z } from "zod";
import { opencodeRegistryRoot } from "../host/roots.ts";
import type { PluginManifest } from "../inventory/types.ts";
import type { OwnedHelper } from "./records.ts";

const HelperSchema = z.strictObject({ path: z.string().min(1), source: z.string().min(1) });

const LedgerSchema = z.strictObject({
  version: z.literal(1),
  plugins: z.record(
    z.string().min(1),
    z.strictObject({ spec: z.string().min(1), helpers: z.array(HelperSchema) }),
  ),
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

/** Remove `helper` while it is still the symlink toolu published; anything else there is the user's. */
function retireHelper(owner: string, helper: OwnedHelper, out: Cleanup): boolean {
  const stat = lstatSync(helper.path, { throwIfNoEntry: false });
  if (stat === undefined) return true;
  if (!stat.isSymbolicLink() || readlinkSync(helper.path) !== helper.source) {
    out.diagnostics.push(`${owner}: kept ${helper.path}, no longer toolu's`);
    return true;
  }
  try {
    rmSync(helper.path);
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
  out: Cleanup,
): OwnedHelper[] {
  return helpers.filter((helper) => !retireHelper(owner, helper, out));
}

function isRegularFile(path: string): boolean {
  return lstatSync(path, { throwIfNoEntry: false })?.isFile() ?? false;
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

/** Remove the registry modules of `spec`, a plugin that is not selected. */
function pruneModules(owner: string, spec: string, dataRoot: string, out: Cleanup): void {
  for (const dir of REGISTRY_DIRS) {
    for (const path of moduleFiles(join(opencodeRegistryRoot(dataRoot), dir), spec)) {
      if (!isRegularFile(path)) continue;
      try {
        rmSync(path);
        out.diagnostics.push(`${owner}: removed module ${path}`);
      } catch (error) {
        out.failures.push(`${owner}: cannot remove module ${path}: ${String(error)}`);
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
  const specs = new Map<string, string>();
  for (const [name, entry] of Object.entries(input.ledger.plugins)) specs.set(name, entry.spec);
  for (const plugin of input.catalog) specs.set(plugin.name, plugin.spec);
  const retained: Ledger["plugins"] = {};
  for (const [name, spec] of specs) {
    if (input.selected.has(name)) continue;
    pruneModules(name, spec, input.dataRoot, out);
    const left = retireHelpers(name, input.ledger.plugins[name]?.helpers ?? [], out);
    if (left.length > 0) retained[name] = { spec, helpers: left };
  }
  return retained;
}
