/** Dependencies start before their dependents, and a cycle has no order (#342). */
import { expect, test } from "bun:test";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";
import { startupOrder } from "../order.ts";
import { PLUGINS_ROOT, fixturePlugin, tempRoot } from "./fixtures.ts";

test.concurrent("the real delivery-flow closure orders every dependency first", () => {
  const selected = selectPluginsByEnabledNames(PLUGINS_ROOT, ["delivery-flow"]);
  if (!selected.ok) throw new Error(selected.reason);
  // Selection is breadth first: the dependent comes before its dependencies.
  expect(selected.plugins[0]?.name).toBe("delivery-flow");
  const order = startupOrder(selected.plugins);
  expect(order.ok && order.plugins.map((p) => p.name)).toEqual([
    "brainstorm",
    "toolu",
    "pr-babysit",
    "toolu-review",
    "delivery-flow",
  ]);
});

test.concurrent("a dependency cycle is named with its path", () => {
  using root = tempRoot("toolu-order-cycle-");
  const a = fixturePlugin(root.path, "a", { dependencies: ["b"] });
  const b = fixturePlugin(root.path, "b", { dependencies: ["a"] });
  expect(startupOrder([b, a])).toEqual({
    ok: false,
    reason: "plugin dependency cycle: a -> b -> a",
  });
});

test.concurrent("a dependency outside the given plugins is a reason", () => {
  using root = tempRoot("toolu-order-missing-");
  const leaf = fixturePlugin(root.path, "leaf", { dependencies: ["core"] });
  expect(startupOrder([leaf])).toEqual({
    ok: false,
    reason: "leaf requires unselected dependency core",
  });
  expect(startupOrder([])).toEqual({ ok: true, plugins: [] });
});
