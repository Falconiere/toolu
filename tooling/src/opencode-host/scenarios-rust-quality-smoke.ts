/** Pinned-host proof of selected Rust quality checks on native OpenCode tools (#354). */
import { join } from "node:path";
import type { PretoolScenario } from "./pretool-shared.ts";
import {
  addedLines,
  disabledQuality,
  editQuality,
  patchQuality,
  qualityProject,
} from "./quality-smoke-shared.ts";

const FILES = {
  "Cargo.toml": '[package]\nname = "rust-quality-smoke"\nversion = "0.1.0"\nedition = "2021"\n',
};
const PROJECT = qualityProject(["rust-quality"], FILES);
const UNWRAP =
  '//! Loader.\n\n/// Load a value.\npub fn load() -> u32 {\n    "1".parse().unwrap()\n}\n';
const SUPPRESSED = "#[allow(dead_code)]\nfn unused() {}\n";
const MOCKED =
  "use mockall::predicate;\n\n#[test]\nfn it_works() {\n    assert!(predicate::eq(1).eval(&1));\n}\n";
const CLEAN = "//! Clean.\n\n/// Answer.\npub fn answer() -> u32 {\n    42\n}\n";
const NOTES = '"1".parse().unwrap()\n';

function seedPatch(project: string): string {
  return [
    "*** Begin Patch",
    `*** Add File: ${join(project, "src/old.rs")}`,
    ...addedLines(UNWRAP),
    `*** Add File: ${join(project, "src/removed.rs")}`,
    ...addedLines(UNWRAP),
    "*** End Patch",
  ].join("\n");
}

function patchText(project: string): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${join(project, "src/old.rs")}`,
    `*** Move to: ${join(project, "src/moved.rs")}`,
    "@@",
    " pub fn load() -> u32 {",
    '-    "1".parse().unwrap()',
    '+    "1".parse().expect("number")',
    " }",
    `*** Delete File: ${join(project, "src/removed.rs")}`,
    `*** Add File: ${join(project, "tests/added.rs")}`,
    ...addedLines(MOCKED),
    `*** Add File: ${join(project, "src/notes.md")}`,
    ...addedLines(NOTES),
    "*** End Patch",
  ].join("\n");
}

export const RUST_QUALITY_SCENARIOS: PretoolScenario[] = [
  {
    id: "rsquality.edit",
    run: (ctx) =>
      editQuality(ctx, {
        id: "rsquality.edit",
        project: PROJECT,
        language: "Rust",
        file: "bad.rs",
        bad: SUPPRESSED,
        clean: CLEAN,
        diagnostic: "Forbidden lint suppression",
      }),
  },
  {
    id: "rsquality.patch",
    run: (ctx) =>
      patchQuality(ctx, {
        id: "rsquality.patch",
        project: PROJECT,
        source: "rust-quality-hook",
        seed: seedPatch,
        patch: patchText,
        moved: {
          from: "src/old.rs",
          to: "src/moved.rs",
          content: UNWRAP.replace(".unwrap()", '.expect("number")'),
        },
        removed: "src/removed.rs",
        added: { path: "tests/added.rs", content: MOCKED },
        notes: { path: "src/notes.md", content: NOTES },
        seedDiagnostics: (project) => [
          `.unwrap() in ${join(project, "src/old.rs")}`,
          `.unwrap() in ${join(project, "src/removed.rs")}`,
        ],
        patchDiagnostics: (project) => [
          `.expect() in ${join(project, "src/moved.rs")}`,
          "no-mocks: mockall/faux import",
        ],
      }),
  },
  {
    id: "rsquality.disabled",
    run: (ctx) =>
      disabledQuality(ctx, {
        id: "rsquality.disabled",
        files: FILES,
        file: "bad.rs",
        content: SUPPRESSED,
      }),
  },
];
