/** Pinned-host proof of selected Python quality checks on native OpenCode tools (#353). */
import { join } from "node:path";
import type { PretoolScenario } from "./pretool-shared.ts";
import {
  addedLines,
  disabledQuality,
  editQuality,
  patchQuality,
  qualityProject,
} from "./quality-smoke-shared.ts";

const FILES = { "pyproject.toml": '[project]\nname = "python-quality-smoke"\n' };
const PROJECT = qualityProject(["python-quality"], FILES);
const BARE = "def load():\n    try:\n        return 1\n    except:\n        return 0\n";
const SWALLOW = "try:\n    run()\nexcept Exception: pass\n";
const MOCKED = "from unittest import mock\n\n\ndef test_it():\n    assert mock\n";
const CLEAN = '"""Clean."""\n\n\ndef answer():\n    """Answer."""\n    return 42\n';

function seedPatch(project: string): string {
  return [
    "*** Begin Patch",
    `*** Add File: ${join(project, "pkg/old.py")}`,
    ...addedLines(BARE),
    `*** Add File: ${join(project, "pkg/removed.py")}`,
    ...addedLines(BARE),
    "*** End Patch",
  ].join("\n");
}

function patchText(project: string): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${join(project, "pkg/old.py")}`,
    `*** Move to: ${join(project, "pkg/moved.py")}`,
    "@@",
    "     try:",
    "         return 1",
    "-    except:",
    "-        return 0",
    "+    except Exception: pass",
    `*** Delete File: ${join(project, "pkg/removed.py")}`,
    `*** Add File: ${join(project, "pkg/test_added.py")}`,
    ...addedLines(MOCKED),
    `*** Add File: ${join(project, "pkg/notes.md")}`,
    "+except: pass",
    "*** End Patch",
  ].join("\n");
}

export const PYTHON_QUALITY_SCENARIOS: PretoolScenario[] = [
  {
    id: "pyquality.edit",
    run: (ctx) =>
      editQuality(ctx, {
        id: "pyquality.edit",
        project: PROJECT,
        language: "Python",
        file: "bad.py",
        bad: SWALLOW,
        clean: CLEAN,
        diagnostic: "Forbidden suppression",
      }),
  },
  {
    id: "pyquality.patch",
    run: (ctx) =>
      patchQuality(ctx, {
        id: "pyquality.patch",
        project: PROJECT,
        source: "python-quality-hook",
        seed: seedPatch,
        patch: patchText,
        moved: {
          from: "pkg/old.py",
          to: "pkg/moved.py",
          content: BARE.replace("    except:\n        return 0\n", "    except Exception: pass\n"),
        },
        removed: "pkg/removed.py",
        added: { path: "pkg/test_added.py", content: MOCKED },
        notes: { path: "pkg/notes.md", content: "except: pass\n" },
        seedDiagnostics: (project) => [
          `${join(project, "pkg/old.py")} `,
          `${join(project, "pkg/removed.py")} `,
        ],
        patchDiagnostics: () => ["one-line except ...: pass", "no-mocks: mock import"],
      }),
  },
  {
    id: "pyquality.disabled",
    run: (ctx) =>
      disabledQuality(ctx, {
        id: "pyquality.disabled",
        files: FILES,
        file: "bad.py",
        content: SWALLOW,
      }),
  },
];
