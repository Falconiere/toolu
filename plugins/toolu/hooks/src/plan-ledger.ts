#!/usr/bin/env bun
/** Stable bundled CLI for delivery-flow's native plan ledger. */
import { ledgerMain } from "@toolu/core/ledger";

const result = await ledgerMain(process.argv.slice(2));
process.stdout.write(result.stdout);
process.stderr.write(result.stderr);
process.exitCode = result.exitCode;
