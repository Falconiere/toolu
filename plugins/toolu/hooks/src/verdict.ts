#!/usr/bin/env bun
/** Stable bundled CLI for delivery-flow's native four-gate verdict. */
import { verdictMain } from "@toolu/core/ledger";

const result = verdictMain(process.argv.slice(2));
process.stdout.write(result.stdout);
process.stderr.write(result.stderr);
process.exitCode = result.exitCode;
