/**
 * Generated blocks of docs/opencode-host-contract.md (#335): the probe
 * results, the 16-plugin matrix, and the limitations list. The checker
 * compares each block with this rendering; `check --write-doc` rewrites them.
 */
import {
  AXES,
  ContractError,
  isUsed,
  workPackageIssue,
  type Cell,
  type Matrix,
  type ProbeResults,
  type UsedCell,
  type WorkPackage,
} from "./schema.ts";

export const BLOCKS = ["probes", "matrix", "limitations"] as const;
export type BlockName = (typeof BLOCKS)[number];

const STATUS_MARK: Record<UsedCell["status"], string> = {
  supported: "✅",
  partial: "🟡",
  unsupported: "❌",
};

function markers(name: BlockName): [string, string] {
  return [`<!-- opencode-host-${name}:start -->`, `<!-- opencode-host-${name}:end -->`];
}

function owners(wps: readonly WorkPackage[]): string {
  return wps.map((wp) => `${wp} (#${workPackageIssue(wp)})`).join(", ");
}

export function renderProbes(results: ProbeResults): string {
  const { host } = results;
  const head = [
    `Recorded ${results.recordedAt} on \`${host.cli}@${host.cliVersion}\` (${host.platform}, Bun ${host.bun}); the host provisioned \`${host.sdk}@${host.provisionedSdkVersion}\`. Install: ${host.installSource}.`,
    "",
    "| Probe | Axis | Kind | Mechanism | Claim | Verdict |",
    "|---|---|---|---|---|---|",
  ];
  const rows = results.probes.map(
    (p) =>
      `| \`${p.id}\` | ${p.axis} | ${p.kind} | ${p.mechanism} | ${p.claim} | ${p.verdict === "supported" ? "✅ supported" : "❌ unsupported"} |`,
  );
  return [...head, ...rows].join("\n");
}

function cellMark(cell: Cell): string {
  if (!isUsed(cell)) return "—";
  return `${STATUS_MARK[cell.status]}${cell.enforcement ? "🔒" : ""}${cell.required ? "" : " (opt)"}`;
}

function detailLines(name: string, row: Matrix["plugins"][string]): string[] {
  const lines = AXES.flatMap((axis) => {
    const cell = row.axes[axis];
    if (!isUsed(cell)) return [];
    const alt = cell.alternative === undefined ? "" : ` Alternative: ${cell.alternative}.`;
    return [
      `  - **${axis}** — ${cell.use}. \`${cell.mechanism}\` (${cell.kind}): ${cell.status}; evidence ${cell.evidence.map((id) => `\`${id}\``).join(", ")}.${alt} Owner: ${owners(cell.owner)}.`,
    ];
  });
  const s = row.surfaces;
  const surface = `  - **surfaces** — ${s.skills} skills, ${s.commands} commands, ${s.agents} agents. Owner: ${owners(s.owner)}.`;
  const notes = (row.notes ?? []).map(
    (note) => `  - **note** — ${note.need}. Owner: ${owners([note.owner])}.`,
  );
  return [`- **${name}** (owner ${owners(row.owner)})`, ...lines, surface, ...notes];
}

export function renderMatrix(matrix: Matrix): string {
  const head = [
    "✅ supported · 🟡 partial · ❌ unsupported · — not needed · 🔒 enforcement (decides whether a tool call runs)",
    "",
    `| Plugin | ${AXES.join(" | ")} | Surfaces (skills/commands/agents) |`,
    `|---|${AXES.map(() => "---").join("|")}|---|`,
  ];
  const entries = Object.entries(matrix.plugins);
  const rows = entries.map(([name, row]) => {
    const s = row.surfaces;
    return `| ${name} | ${AXES.map((axis) => cellMark(row.axes[axis])).join(" | ")} | ${s.skills}/${s.commands}/${s.agents} |`;
  });
  return [...head, ...rows, "", ...entries.flatMap(([name, row]) => detailLines(name, row))].join(
    "\n",
  );
}

function requiredCells(matrix: Matrix): Array<{ ref: string; cell: UsedCell }> {
  return Object.entries(matrix.plugins).flatMap(([name, row]) =>
    AXES.flatMap((axis) => {
      const cell = row.axes[axis];
      return isUsed(cell) && cell.required ? [{ ref: `${name}.${axis}`, cell }] : [];
    }),
  );
}

export function renderLimitations(matrix: Matrix): string {
  const required = requiredCells(matrix);
  const blockers = required.filter(({ cell }) => cell.releaseBlocker === true);
  const limited = required.filter(
    ({ cell }) => cell.status !== "supported" && cell.releaseBlocker !== true,
  );
  const experimental = [
    ...new Set(
      required.flatMap(({ cell }) => cell.mechanism.match(/experimental\.[a-z.]+/g) ?? []),
    ),
  ].toSorted();
  return [
    "### Release blockers",
    "",
    ...(blockers.length === 0
      ? [
          "None. Every required capability is supported on the pinned host or has an alternative backed by supported probes.",
        ]
      : blockers.map(
          ({ ref, cell }) =>
            `- \`${ref}\` (${cell.status}) — ${cell.use}. Owner: ${owners(cell.owner)}.`,
        )),
    "",
    "### Limitations and alternatives",
    "",
    ...limited.map(
      ({ ref, cell }) =>
        `- \`${ref}\` (${cell.status}) — ${cell.use}. Alternative: ${cell.alternative ?? ""}. Evidence: ${(cell.alternativeEvidence ?? cell.evidence).map((id) => `\`${id}\``).join(", ")}. Owner: ${owners(cell.owner)}.`,
    ),
    "",
    "### Host constraints",
    "",
    ...matrix.host.map(
      (c) =>
        `- ${c.need}. Evidence: ${c.evidence.map((id) => `\`${id}\``).join(", ")}. Owner: ${owners(c.owner)}.`,
    ),
    "",
    "### Experimental hooks under the exact pin",
    "",
    ...experimental.map(
      (hook) =>
        `- \`${hook}\`: ${required
          .filter(({ cell }) => cell.mechanism.includes(hook))
          .map(({ ref }) => `\`${ref}\``)
          .join(", ")}`,
    ),
  ].join("\n");
}

/** The current text between a block's markers. */
export function readBlock(doc: string, name: BlockName): string {
  const [start, end] = markers(name);
  const from = doc.indexOf(start);
  const to = doc.indexOf(end);
  if (from < 0 || to < from) throw new ContractError(`doc block ${name} markers missing`);
  return doc.slice(from + start.length, to).trim();
}

export function writeBlock(doc: string, name: BlockName, body: string): string {
  const [start, end] = markers(name);
  const from = doc.indexOf(start);
  const to = doc.indexOf(end);
  if (from < 0 || to < from) throw new ContractError(`doc block ${name} markers missing`);
  return `${doc.slice(0, from + start.length)}\n${body}\n${doc.slice(to)}`;
}
