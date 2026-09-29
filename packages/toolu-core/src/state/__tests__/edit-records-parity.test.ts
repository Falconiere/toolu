/**
 * `normalizeEditRecords`/`formatEditRecords` vs bash
 * `toolu_normalize_edit_records` (#255, AC-7). Every payload from
 * edit-records.bats, plus edge fixtures, runs through both. Stdout bytes and
 * the outcome (exit 0 records, 1 not an edit tool, 2 malformed) must match.
 */
import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { formatEditRecords, isEditTool, normalizeEditRecords } from "../edit-records.ts";
import { EditRecordSchema } from "../state-schema.ts";

const EDIT_RECORDS_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/edit-records.sh",
);

const patch = (command: unknown) => ({ tool_name: "apply_patch", tool_input: { command } });
const edit = (tool: string, toolInput: unknown) => ({ tool_name: tool, tool_input: toolInput });

const MOVE_PATCH = `*** Begin Patch
*** Add File: src/new file.ts
+export const x = 1
*** Update File: src/old.ts
@@
-old
+new
*** Delete File: src/gone.rs
*** Update File: src/from.ts
*** Move to: src/to.ts
@@
-before
+after
*** End Patch`;

const CASES: [string, string, unknown][] = [
  // edit-records.bats
  ["bats: Claude Edit", "Edit", { tool_name: "Edit", tool_input: { file_path: "src/main.ts" } }],
  ["bats: add, update, delete and a move", "apply_patch", patch(MOVE_PATCH)],
  [
    "bats: multiple updates in order",
    "apply_patch",
    patch(
      "*** Begin Patch\n*** Update File: a.ts\n@@\n-a\n+b\n*** Update File: b.rs\n@@\n-c\n+d\n*** End Patch",
    ),
  ],
  [
    "bats: End of File marker",
    "apply_patch",
    patch(
      "*** Begin Patch\n*** Update File: src/main.ts\n@@\n-old\n+new\n*** End of File\n*** End Patch",
    ),
  ],
  ["bats: no Begin Patch", "apply_patch", patch("*** Add File: a.ts\n+x\n*** End Patch")],
  ["bats: no End Patch", "apply_patch", patch("*** Begin Patch\n*** Update File: a.ts")],
  [
    "bats: Move to without an update",
    "apply_patch",
    patch("*** Begin Patch\n*** Move to: b.ts\n*** End Patch"),
  ],
  [
    "bats: empty Add File path",
    "apply_patch",
    patch("*** Begin Patch\n*** Add File: \n*** End Patch"),
  ],
  ["bats: no headers", "apply_patch", patch("*** Begin Patch\n*** End Patch")],
  [
    "bats: Bash is not an edit tool",
    "Bash",
    { tool_name: "Bash", tool_input: { command: "true" } },
  ],
  // Edit / Write / MultiEdit path fallbacks
  ["Write", "Write", edit("Write", { file_path: "out.txt", content: "x" })],
  ["MultiEdit", "MultiEdit", edit("MultiEdit", { file_path: "m.ts", edits: [] })],
  ["path key", "Edit", edit("Edit", { path: "p.ts" })],
  ["target_file key", "Edit", edit("Edit", { target_file: "t.ts" })],
  [
    "file_path false falls through to path",
    "Edit",
    edit("Edit", { file_path: false, path: "p.ts" }),
  ],
  [
    "file_path null and path null fall through to target_file",
    "Edit",
    edit("Edit", { file_path: null, path: null, target_file: "t.ts" }),
  ],
  [
    "empty file_path is selected, then invalid",
    "Edit",
    edit("Edit", { file_path: "", path: "p.ts" }),
  ],
  ["numeric file_path", "Edit", edit("Edit", { file_path: 5 })],
  ["no path at all", "Edit", edit("Edit", {})],
  ["trailing newlines are stripped by $(...)", "Write", edit("Write", { file_path: "a.ts\n\n" })],
  ["CR in path", "Edit", edit("Edit", { file_path: "a\r.ts" })],
  ["TAB in path", "Edit", edit("Edit", { file_path: "a\tb.ts" })],
  ["inner newline in path", "Edit", edit("Edit", { file_path: "a\nb.ts" })],
  ["DEL and unicode in path", "Edit", edit("Edit", { file_path: "src/é ～ \u007f.ts" })],
  ["tool_input is a string", "Edit", edit("Edit", "src/a.ts")],
  ["tool_input is false", "Edit", edit("Edit", false)],
  ["tool_input absent", "Edit", { tool_name: "Edit" }],
  ["payload is an array", "Edit", ["Edit"]],
  ["payload is null", "Write", null],
  // apply_patch edges
  [
    "CRLF patch",
    "apply_patch",
    patch("*** Begin Patch\r\n*** Update File: a.ts\r\n@@\r\n-a\r\n+b\r\n*** End Patch\r\n"),
  ],
  [
    "trailing newlines after End Patch",
    "apply_patch",
    patch("*** Begin Patch\n*** Delete File: a.ts\n*** End Patch\n\n\n"),
  ],
  [
    "text after End Patch",
    "apply_patch",
    patch("*** Begin Patch\n*** Delete File: a.ts\n*** End Patch\nrm -rf /"),
  ],
  [
    "unknown control header",
    "apply_patch",
    patch("*** Begin Patch\n*** Rename File: a.ts\n*** End Patch"),
  ],
  [
    "Begin Patch twice",
    "apply_patch",
    patch("*** Begin Patch\n*** Begin Patch\n*** Delete File: a.ts\n*** End Patch"),
  ],
  [
    "End of File before any header",
    "apply_patch",
    patch("*** Begin Patch\n*** End of File\n*** Delete File: a.ts\n*** End Patch"),
  ],
  [
    "header without its space",
    "apply_patch",
    patch("*** Begin Patch\n*** Delete File:a.ts\n*** End Patch"),
  ],
  [
    "path keeps a second leading space and trailing spaces",
    "apply_patch",
    patch("*** Begin Patch\n*** Add File:  a.ts \n*** End Patch"),
  ],
  [
    "move of the last update, then another file",
    "apply_patch",
    patch(
      "*** Begin Patch\n*** Update File: a.ts\n*** Move to: b.ts\n*** Add File: c.ts\n*** End Patch",
    ),
  ],
  [
    "Move to with an invalid target",
    "apply_patch",
    patch("*** Begin Patch\n*** Update File: a.ts\n*** Move to: \n*** End Patch"),
  ],
  ["non-string command", "apply_patch", patch(["apply_patch", "x"])],
  ["empty command", "apply_patch", patch("")],
  ["missing command", "apply_patch", edit("apply_patch", {})],
  ["empty tool name", "", edit("", { file_path: "a.ts" })],
];

const EXIT: Record<"records" | "not-edit" | "malformed", number> = {
  records: 0,
  "not-edit": 1,
  malformed: 2,
};

async function bash(input: string, tool: string): Promise<{ code: number; stdout: string }> {
  const res = await run([
    "bash",
    "-c",
    '. "$1"; toolu_normalize_edit_records "$2" "$3"',
    "_",
    EDIT_RECORDS_SH,
    input,
    tool,
  ]);
  return { code: res.exitCode, stdout: res.stdout };
}

describe("TypeScript matches bash", () => {
  for (const [name, tool, payload] of CASES) {
    test(name, async () => {
      const input = JSON.stringify(payload);
      const result = normalizeEditRecords(JSON.parse(input), tool);
      const ts = {
        code: EXIT[result.kind],
        stdout: result.kind === "records" ? formatEditRecords(result.records) : "",
      };
      expect(ts).toEqual(await bash(input, tool));
      if (result.kind === "records") {
        for (const record of result.records)
          expect(EditRecordSchema.safeParse(record).success).toBe(true);
      }
    });
  }
});

test("the move fixture yields both sides of the move", () => {
  expect(normalizeEditRecords(patch(MOVE_PATCH), "apply_patch")).toEqual({
    kind: "records",
    records: [
      { path: "src/new file.ts", operation: "add" },
      { path: "src/old.ts", operation: "update" },
      { path: "src/gone.rs", operation: "delete" },
      { path: "src/from.ts", operation: "update", moved_to: "src/to.ts" },
      { path: "src/to.ts", operation: "move", from: "src/from.ts" },
    ],
  });
});

test.each([
  ["Edit", true],
  ["Write", true],
  ["MultiEdit", true],
  ["apply_patch", true],
  ["Bash", false],
  ["edit", false],
])("isEditTool(%s) is %p", (tool, expected) => {
  expect(isEditTool(tool)).toBe(expected);
});
