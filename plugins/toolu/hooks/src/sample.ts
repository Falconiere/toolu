/**
 * Bundle-pipeline canary (#249): proves a hook entry that imports @toolu/core
 * bundles to a self-contained plugins/toolu/hooks/dist/sample.js that runs with
 * `bun` and no node_modules. No hook invokes it; later ports replace it.
 */
import { parseDecision } from "@toolu/core/decision";

process.stdout.write(`${JSON.stringify(parseDecision({ kind: "allow" }))}\n`);
