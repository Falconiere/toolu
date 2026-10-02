import { expect, test } from "bun:test";
import { cpSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { CATALOG_FILE, readSurfaceCatalog } from "../catalog.ts";

const GENERATED = realpathSync(join(import.meta.dir, "../../../generated"));
const tmpBase = process.env.TMPDIR ?? "/tmp";

/** A real copy of the committed generated tree that a test may corrupt. */
function copy(): string {
  const dir = join(realpathSync(mkdtempSync(join(tmpBase, "toolu-catalog-"))), "generated");
  cpSync(GENERATED, dir, { recursive: true });
  return dir;
}

const RawSchema = z.looseObject({
  plugins: z.array(
    z.looseObject({
      name: z.string(),
      skills: z.array(z.looseObject({ id: z.string(), path: z.string() })),
    }),
  ),
});
type Raw = z.infer<typeof RawSchema>;
type RawSkill = Raw["plugins"][number]["skills"][number];

function editCatalog(dir: string, edit: (raw: Raw) => void): void {
  const path = join(dir, CATALOG_FILE);
  const raw = RawSchema.parse(JSON.parse(readFileSync(path, "utf8")));
  edit(raw);
  writeFileSync(path, JSON.stringify(raw));
}

function tooluSkills(raw: Raw): RawSkill[] {
  const plugin = raw.plugins.find((p) => p.name === "toolu");
  if (plugin === undefined) throw new Error("toolu missing from the committed catalog");
  return plugin.skills;
}

/** Edit the first toolu skill entry in the catalog copy at `dir`. */
function editFirstSkill(dir: string, edit: (skill: RawSkill, skills: RawSkill[]) => void): void {
  editCatalog(dir, (raw) => {
    const skills = tooluSkills(raw);
    const first = skills[0];
    if (first === undefined) throw new Error("toolu has no skills");
    edit(first, skills);
  });
}

function reasonOf(dir: string): string {
  const catalog = readSurfaceCatalog(dir);
  if (catalog.ok) throw new Error("expected an invalid catalog");
  return catalog.reason;
}

test("the committed catalog validates with every file inside generated/", () => {
  const catalog = readSurfaceCatalog(GENERATED);
  if (!catalog.ok) throw new Error(catalog.reason);
  expect(catalog.plugins).toHaveLength(16);
  const core = catalog.plugins.find((p) => p.name === "toolu");
  expect(core?.skills.map((s) => s.id)).toContain("toolu-debug");
  expect(core?.agents).toHaveLength(5);
  for (const plugin of catalog.plugins)
    for (const entry of [...plugin.skills, ...plugin.agents, ...plugin.commands])
      expect(entry.file.startsWith(`${GENERATED}/`)).toBe(true);
});

test("every catalog corruption is a reason naming the problem", () => {
  const cases: Array<[string, (dir: string) => void, string]> = [
    [
      "missing",
      (dir) => rmSync(join(dir, "agents/toolu-quick-task.md")),
      "agents toolu-quick-task: missing file",
    ],
    [
      "duplicate",
      (dir) => editFirstSkill(dir, (first, skills) => skills.push({ ...first })),
      "duplicate ID",
    ],
    [
      "escape",
      (dir) =>
        editFirstSkill(dir, (first) => {
          first.path = "../../etc/passwd";
        }),
      "escapes the generated directory",
    ],
    [
      "shape",
      (dir) =>
        editFirstSkill(dir, (first) => {
          first.path = "agents/toolu-quick-task.md";
        }),
      "expected skills/",
    ],
    [
      "bad-id",
      (dir) =>
        editFirstSkill(dir, (first) => {
          first.id = "toolu--debug";
        }),
      "single hyphens",
    ],
    [
      "renamed",
      (dir) =>
        writeFileSync(
          join(dir, "skills/toolu-debug/SKILL.md"),
          "---\nname: toolu-other\ndescription: d\n---\n",
        ),
      "SKILL.md name is not toolu-debug",
    ],
    ["json", (dir) => writeFileSync(join(dir, CATALOG_FILE), "{ nope"), "catalog "],
    [
      "version",
      (dir) => writeFileSync(join(dir, CATALOG_FILE), JSON.stringify({ version: 2, plugins: [] })),
      "invalid ",
    ],
    ["absent", (dir) => rmSync(join(dir, CATALOG_FILE)), "catalog "],
  ];
  for (const [label, corrupt, expected] of cases) {
    const dir = copy();
    corrupt(dir);
    expect({ label, reason: reasonOf(dir).includes(expected) }).toEqual({ label, reason: true });
  }
});
