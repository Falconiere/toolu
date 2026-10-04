/**
 * Hermetic consistency rules for the OpenCode host contract (#335). Each rule
 * throws a ContractError naming what disagrees: pins, committed live evidence,
 * the 16-plugin matrix, the catalog's own manifests, the installed SDK
 * declarations and the contract doc.
 */
import { join } from "node:path";
import { catalogPlugins, derivedAxes, pluginSurfaces } from "./manifests.ts";
import {
  AXES,
  ContractError,
  PROBE_IDS,
  isUsed,
  VerdictsSchema,
  type Cell,
  type Matrix,
  type Pin,
  type ProbeId,
  type ProbeResults,
  type UsedCell,
  type Verdict,
  type Verdicts,
} from "./schema.ts";

/** Check the pins and that the results hold each probe once; return the verdict per probe. */
/** The adapter's SDK declarations: dev dependencies for typechecking, optional peers for consumers. */
export type AdapterSdk = {
  devDependencies?: Record<string, string> | undefined;
  peerDependencies?: Record<string, string> | undefined;
};

const SDK_PACKAGES = ["@opencode-ai/plugin", "@opencode-ai/sdk"];
const SDK_DECLARATIONS = [
  ["devDependencies", "devDependency"],
  ["peerDependencies", "peerDependency"],
] as const;

/** Every SDK package the adapter declares, as a dev dependency and as a peer, equals the pin. */
function checkAdapterSdk(pin: Pin, adapter: AdapterSdk): void {
  for (const [field, label] of SDK_DECLARATIONS) {
    for (const name of SDK_PACKAGES) {
      const declared = adapter[field]?.[name];
      if (declared !== pin.sdk.version) {
        throw new ContractError(
          `pin mismatch: @toolu/opencode ${label} ${name} is ${declared ?? "absent"}, pin is ${pin.sdk.version}`,
        );
      }
    }
  }
}

export function checkPins(pin: Pin, adapter: AdapterSdk, results: ProbeResults): Verdicts {
  checkAdapterSdk(pin, adapter);
  if (results.host.cliVersion !== pin.cli.version) {
    throw new ContractError(
      `pin mismatch: probe results recorded opencode-ai ${results.host.cliVersion}, pin is ${pin.cli.version}`,
    );
  }
  if (results.host.provisionedSdkVersion !== pin.sdk.version) {
    throw new ContractError(
      `pin mismatch: probe results recorded @opencode-ai/plugin ${results.host.provisionedSdkVersion}, pin is ${pin.sdk.version}`,
    );
  }
  const entries = PROBE_IDS.map((id): [ProbeId, Verdict] => {
    const found = results.probes.filter((p) => p.id === id);
    const [only] = found;
    if (found.length !== 1 || only === undefined) {
      throw new ContractError(
        `probe results must contain ${id} exactly once (found ${found.length})`,
      );
    }
    return [id, only.verdict];
  });
  return VerdictsSchema.parse(Object.fromEntries(entries));
}

function checkCell(ref: string, cell: UsedCell, verdicts: Verdicts): void {
  const seen = cell.evidence.map((id) => verdicts[id]);
  const consistent =
    cell.status === "supported"
      ? seen.every((v) => v === "supported")
      : cell.status === "unsupported"
        ? seen.every((v) => v === "unsupported")
        : seen.includes("supported") && seen.includes("unsupported");
  if (!consistent) throw new ContractError(`status ${cell.status} contradicts evidence for ${ref}`);
  if (!cell.required || cell.status === "supported" || cell.releaseBlocker === true) return;
  if (cell.alternative === undefined) {
    throw new ContractError(
      `required ${ref} is ${cell.status} without alternative or releaseBlocker`,
    );
  }
  const alternativeEvidence = cell.alternativeEvidence ?? [];
  const altVerified =
    alternativeEvidence.length > 0 &&
    alternativeEvidence.every((id) => verdicts[id] === "supported");
  if (cell.enforcement && !altVerified) {
    throw new ContractError(
      `enforcement ${ref} needs supported alternativeEvidence or releaseBlocker`,
    );
  }
}

function checkRow(
  name: string,
  pluginDir: string,
  row: Matrix["plugins"][string],
  verdicts: Verdicts,
): void {
  const axes: Record<string, Cell> = row.axes;
  for (const axis of derivedAxes(pluginDir)) {
    if (axes[axis]?.use === "none")
      throw new ContractError(`${name}.${axis}: manifests need it but the matrix says none`);
  }
  for (const axis of AXES) {
    const cell = row.axes[axis];
    if (isUsed(cell)) checkCell(`${name}.${axis}`, cell, verdicts);
  }
  const disk = pluginSurfaces(pluginDir);
  for (const kind of ["skills", "commands", "agents"] as const) {
    if (row.surfaces[kind] !== disk[kind]) {
      throw new ContractError(
        `${name}.surfaces.${kind} is ${row.surfaces[kind]}, plugins/${name} has ${disk[kind]}`,
      );
    }
  }
}

function cited(matrix: Matrix): Set<ProbeId> {
  const fromCells = Object.values(matrix.plugins).flatMap((row) =>
    AXES.flatMap((axis) => {
      const cell = row.axes[axis];
      return isUsed(cell) ? cell.evidence.concat(cell.alternativeEvidence ?? []) : [];
    }),
  );
  return new Set([...fromCells, ...matrix.host.flatMap((c) => c.evidence)]);
}

export function checkMatrix(matrix: Matrix, verdicts: Verdicts, pluginsDir: string): void {
  const catalog = new Set(catalogPlugins(pluginsDir));
  for (const name of catalog) {
    if (matrix.plugins[name] === undefined) throw new ContractError(`missing matrix row: ${name}`);
  }
  for (const [name, row] of Object.entries(matrix.plugins)) {
    if (!catalog.has(name)) throw new ContractError(`unknown plugin: ${name}`);
    checkRow(name, join(pluginsDir, name), row, verdicts);
  }
  const citedIds = cited(matrix);
  const orphan = PROBE_IDS.find((id) => verdicts[id] === "unsupported" && !citedIds.has(id));
  if (orphan !== undefined)
    throw new ContractError(`unsupported probe ${orphan} has no owner in the matrix`);
}

/** The superseded V2 plugin docs; the documented contract is opencode.ai/docs/plugins/. */
export const V2_DOCS = "opencode.ai/v2/";

export function checkDocText(name: string, doc: string): void {
  if (doc.includes(V2_DOCS)) throw new ContractError(`${name} cites the V2 contract (${V2_DOCS})`);
}

/** Every declared `Hooks` member must appear in a code span of the doc's `## Host surface` section. */
export function checkHostSurface(doc: string, hooks: readonly string[]): void {
  const start = doc.indexOf("## Host surface");
  if (start < 0) throw new ContractError("doc has no ## Host surface section");
  const next = doc.indexOf("\n## ", start + 1);
  const spans = new Set(doc.slice(start, next < 0 ? undefined : next).match(/`[^`]+`/g) ?? []);
  const missing = hooks.find((hook) => !spans.has(`\`${hook}\``));
  if (missing !== undefined) throw new ContractError(`host surface misses hook ${missing}`);
}
