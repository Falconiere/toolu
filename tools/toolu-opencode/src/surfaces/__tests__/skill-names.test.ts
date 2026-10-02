import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { existingSkillNames, type SkillScanScope } from "../skill-names.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function skill(name: string): string {
  return `---\nname: ${name}\ndescription: ${name} skill\n---\nBody\n`;
}

function put(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

type Tree = {
  base: string;
  home: string;
  xdg: string;
  custom: string;
  project: string;
  sub: string;
};

/** A real tree: HOME, XDG config, OPENCODE_CONFIG_DIR, and a project with an instance subdirectory. */
function tree(): Tree {
  const base = realpathSync(mkdtempSync(join(tmpBase, "toolu-skill-names-")));
  const t = {
    base,
    home: join(base, "home"),
    xdg: join(base, "xdg"),
    custom: join(base, "custom"),
    project: join(base, "outer/project"),
    sub: join(base, "outer/project/packages/app"),
  };
  put(join(t.home, ".claude/skills/a/SKILL.md"), skill("home-claude"));
  put(join(t.home, ".agents/skills/b/SKILL.md"), skill("home-agents"));
  put(join(t.home, ".claude/skills/.hidden/m/SKILL.md"), skill("home-claude-dot"));
  put(join(t.project, ".claude/skills/c/SKILL.md"), skill("project-claude"));
  put(join(t.sub, ".agents/skills/d/SKILL.md"), skill("sub-agents"));
  put(join(base, "outer/.claude/skills/e/SKILL.md"), skill("above-worktree"));
  put(join(t.xdg, "opencode/skills/f/SKILL.md"), skill("xdg-skills"));
  put(join(t.xdg, "opencode/skill/g/SKILL.md"), skill("xdg-skill"));
  put(join(t.project, ".opencode/skills/h/SKILL.md"), skill("project-opencode"));
  put(join(t.sub, ".opencode/skill/i/SKILL.md"), skill("sub-opencode"));
  put(join(t.project, ".opencode/skills/.hidden/n/SKILL.md"), skill("project-opencode-dot"));
  put(join(t.home, ".opencode/skills/j/SKILL.md"), skill("home-opencode"));
  put(join(t.custom, "skills/k/SKILL.md"), skill("custom-dir"));
  put(join(base, "abs/nested/l/SKILL.md"), skill("configured-abs"));
  put(join(base, "abs/.hidden/o/SKILL.md"), skill("configured-dot"));
  put(join(t.sub, "rel/p/SKILL.md"), skill("configured-rel"));
  put(join(t.home, "home-skills/q/SKILL.md"), skill("configured-home"));
  return t;
}

function scope(t: Tree, env: Record<string, string> = {}, worktree = t.project): SkillScanScope {
  return {
    directory: t.sub,
    worktree,
    env: { HOME: t.home, XDG_CONFIG_HOME: t.xdg, OPENCODE_CONFIG_DIR: t.custom, ...env },
    configuredPaths: [join(t.base, "abs"), "rel", "~/home-skills"],
  };
}

function names(s: SkillScanScope): string[] {
  return [...existingSkillNames(s).keys()].toSorted();
}

const ALL_SCANNED = [
  "configured-abs",
  "configured-home",
  "configured-rel",
  "custom-dir",
  "home-agents",
  "home-claude",
  "home-claude-dot",
  "home-opencode",
  "project-claude",
  "project-opencode",
  "sub-agents",
  "sub-opencode",
  "xdg-skill",
  "xdg-skills",
];

test("every host root is scanned; dot entries only in .claude/.agents; nothing above the worktree", () => {
  const t = tree();
  expect(names(scope(t))).toEqual(ALL_SCANNED);
  expect(existingSkillNames(scope(t)).get("sub-opencode")).toBe(
    join(t.sub, ".opencode/skill/i/SKILL.md"),
  );
});

test("a '/' worktree walks to the filesystem root", () => {
  const t = tree();
  expect(names(scope(t, {}, "/"))).toContain("above-worktree");
});

test("flags drop roots exactly as the host parses them", () => {
  const t = tree();
  const external = [
    "home-agents",
    "home-claude",
    "home-claude-dot",
    "project-claude",
    "sub-agents",
  ];
  const claude = ["home-claude", "home-claude-dot", "project-claude"];
  const project = ["project-opencode", "sub-opencode"];
  const without = (dropped: string[]): string[] => ALL_SCANNED.filter((n) => !dropped.includes(n));
  for (const value of ["true", "yes", "on", "1", "y"])
    expect(names(scope(t, { OPENCODE_DISABLE_EXTERNAL_SKILLS: value }))).toEqual(without(external));
  for (const value of ["0", "false", "no", "TRUE"])
    expect(names(scope(t, { OPENCODE_DISABLE_EXTERNAL_SKILLS: value }))).toEqual(ALL_SCANNED);
  expect(names(scope(t, { OPENCODE_DISABLE_CLAUDE_CODE: "1" }))).toEqual(without(claude));
  expect(names(scope(t, { OPENCODE_DISABLE_CLAUDE_CODE_SKILLS: "true" }))).toEqual(without(claude));
  expect(names(scope(t, { OPENCODE_DISABLE_PROJECT_CONFIG: "TRUE" }))).toEqual(without(project));
  expect(names(scope(t, { OPENCODE_DISABLE_PROJECT_CONFIG: "1" }))).toEqual(without(project));
  expect(names(scope(t, { OPENCODE_DISABLE_PROJECT_CONFIG: "yes" }))).toEqual(ALL_SCANNED);
});

test("the first root in host order wins a shared name", () => {
  const t = tree();
  put(join(t.home, ".claude/skills/dup/SKILL.md"), skill("toolu-debug"));
  put(join(t.project, ".opencode/skills/dup/SKILL.md"), skill("toolu-debug"));
  expect(existingSkillNames(scope(t)).get("toolu-debug")).toBe(
    join(t.home, ".claude/skills/dup/SKILL.md"),
  );
});

test("files are read the way the host reads them", () => {
  const t = tree();
  const dir = join(t.project, ".opencode/skills");
  put(join(dir, "bom/SKILL.md"), "﻿---\r\nname: bom-crlf\r\ndescription: d\r\n---\r\n");
  put(join(dir, "dup/SKILL.md"), "---\nname: dup-a\nname: dup-b\n---\n");
  put(join(dir, "num/SKILL.md"), "---\nname: 42\n---\n");
  put(join(dir, "lower/skill.md"), skill("lowercase-file"));
  const found = names(scope(t));
  expect(found).toContain("bom-crlf");
  expect(found.filter((n) => n.startsWith("dup-") || n === "42" || n === "lowercase-file")).toEqual(
    [],
  );
});

test("symlinked skill directories are followed and a link loop ends", () => {
  const t = tree();
  const outside = join(t.base, "elsewhere");
  put(join(outside, "x/SKILL.md"), skill("linked-skill"));
  symlinkSync(outside, join(t.project, ".opencode/skills/linked"));
  symlinkSync(join(t.project, ".opencode/skills"), join(t.project, ".opencode/skills/loop"));
  expect(names(scope(t))).toContain("linked-skill");
});

test("missing roots and an unreadable configured path are skipped", () => {
  const base = realpathSync(mkdtempSync(join(tmpBase, "toolu-skill-empty-")));
  const empty: SkillScanScope = {
    directory: base,
    worktree: base,
    env: { HOME: join(base, "nohome"), XDG_CONFIG_HOME: join(base, "noxdg") },
    configuredPaths: [join(base, "missing"), "also-missing"],
  };
  expect(existingSkillNames(empty).size).toBe(0);
});

test("an empty XDG_CONFIG_HOME falls back to ~/.config, as the host's xdg-basedir does", () => {
  const t = tree();
  put(join(t.home, ".config/opencode/skills/r/SKILL.md"), skill("home-config"));
  expect(names(scope(t, { XDG_CONFIG_HOME: "" }))).toContain("home-config");
  expect(names(scope(t))).not.toContain("home-config");
});
