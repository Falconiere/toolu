#!/usr/bin/env bun
import { parseVerdict } from "./babysit/verdict";

const input = await Bun.stdin.text();
process.stdout.write(`${JSON.stringify(parseVerdict(input))}\n`);
