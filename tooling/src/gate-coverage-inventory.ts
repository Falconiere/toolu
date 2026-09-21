#!/usr/bin/env bun
/**
 * Gate coverage inventory CLI for #209.
 *
 * Usage:
 *   bun run tooling/src/gate-coverage-inventory.ts discover|check|render|seed
 */
import { check, loadInventory, render, seed } from "./gate-coverage/check.ts";
import { discover } from "./gate-coverage/discover.ts";
import { fail } from "./gate-coverage/fs-util.ts";

const cmd = process.argv[2] ?? "check";
switch (cmd) {
  case "discover":
    process.stdout.write(`${JSON.stringify(discover(), null, 2)}\n`);
    break;
  case "check":
    check();
    break;
  case "render":
    render(loadInventory());
    break;
  case "seed":
    seed();
    break;
  default:
    fail(`unknown command ${cmd} (discover|check|render|seed)`);
}
