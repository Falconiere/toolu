/** Scenario selection for `bun run smoke:opencode-entry [<id>…]` (#343). */
import { expect, test } from "bun:test";
import { chosen } from "../opencode-entry-smoke.ts";
import { ContractError } from "../opencode-host/schema.ts";

test("no ids runs every scenario; ids narrow it in catalog order", () => {
  const all = chosen([]).map((scenario) => scenario.id);
  expect(all).toContain("entry.npm-root");
  expect(all).toContain("entry.full-startup");
  expect(all).toContain("entry.worktree-state");
  expect(all.slice(-6)).toEqual([
    "surfaces.npm-clean",
    "surfaces.lifecycle",
    "surfaces.precedence",
    "surfaces.skill-roots",
    "surfaces.both-routes",
    "surfaces.selection",
  ]);
  const picked = chosen(["entry.worktree-state", "entry.helper-env"]).map((s) => s.id);
  expect(picked).toEqual(["entry.helper-env", "entry.worktree-state"]);
});

test("an unknown id is an error, never an empty run", () => {
  expect(() => chosen(["entry.helper-env", "entry.nope"])).toThrow(ContractError);
  expect(() => chosen(["entry.nope"])).toThrow("unknown scenario: entry.nope");
});
