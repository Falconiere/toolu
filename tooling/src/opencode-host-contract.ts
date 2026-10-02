#!/usr/bin/env bun
/**
 * Hermetic OpenCode host-contract check (#335): `bun run check:opencode-host`.
 *
 * Verifies that the pin, the committed live-probe evidence, the 16-plugin
 * capability matrix, the catalog manifests, the installed pinned SDK
 * declarations and docs/opencode-host-contract.md agree. `--write-doc`
 * regenerates the doc's blocks first. Live probing is `probe:opencode-host`.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { envOr } from "./env.ts";
import {
  checkDocText,
  checkHostSurface,
  checkMatrix,
  checkPins,
} from "./opencode-host/contract-check.ts";
import { declaredHooks, installedSdk } from "./opencode-host/declarations.ts";
import {
  BLOCKS,
  readBlock,
  renderLimitations,
  renderMatrix,
  renderProbes,
  writeBlock,
  type BlockName,
} from "./opencode-host/doc-blocks.ts";
import { contractPaths, ROOT } from "./opencode-host/results.ts";
import {
  ContractError,
  MatrixSchema,
  PinSchema,
  ProbeResultsSchema,
  readJson,
} from "./opencode-host/schema.ts";

const AdapterPackage = z.looseObject({
  devDependencies: z.record(z.string(), z.string()).optional(),
});

function main(argv: string[]): void {
  const env = process.env;
  const paths = contractPaths(env);
  const docPath = envOr(
    "TOOLU_OPENCODE_CONTRACT_DOC",
    join(ROOT, "docs/opencode-host-contract.md"),
    env,
  );
  const portableDoc = envOr("PORTABLE_CORE_DOC", join(ROOT, "docs/portable-core.md"), env);
  const adapterPkg = envOr(
    "TOOLU_OPENCODE_ADAPTER_PKG",
    join(ROOT, "tools/toolu-opencode/package.json"),
    env,
  );
  const pluginsDir = envOr("TOOLU_PLUGINS_DIR", join(ROOT, "plugins"), env);

  const pin = readJson(paths.pin, PinSchema);
  const results = readJson(paths.results, ProbeResultsSchema);
  const matrix = readJson(paths.matrix, MatrixSchema);
  const adapter = AdapterPackage.parse(JSON.parse(readFileSync(adapterPkg, "utf8")));
  const verdicts = checkPins(pin, adapter.devDependencies?.["@opencode-ai/plugin"], results);
  checkMatrix(matrix, verdicts, pluginsDir);

  const sdk = installedSdk(adapterPkg, ROOT);
  if (sdk.version !== pin.sdk.version) {
    throw new ContractError(
      `pin mismatch: installed @opencode-ai/plugin is ${sdk.version}, pin is ${pin.sdk.version}`,
    );
  }
  const rendered: Record<BlockName, string> = {
    probes: renderProbes(results),
    matrix: renderMatrix(matrix),
    limitations: renderLimitations(matrix),
  };
  let doc = readFileSync(docPath, "utf8");
  if (argv.includes("--write-doc")) {
    for (const name of BLOCKS) doc = writeBlock(doc, name, rendered[name]);
    writeFileSync(docPath, doc);
  }
  for (const name of BLOCKS) {
    if (readBlock(doc, name) !== rendered[name])
      throw new ContractError(`doc block ${name} is stale; run check --write-doc`);
  }
  checkHostSurface(doc, declaredHooks(sdk));
  checkDocText("docs/opencode-host-contract.md", doc);
  checkDocText("docs/portable-core.md", readFileSync(portableDoc, "utf8"));
}

if (import.meta.main) {
  try {
    main(process.argv.slice(2).filter((arg) => arg !== "check"));
    process.stdout.write("opencode-host-contract: ok\n");
  } catch (err: unknown) {
    if (!(err instanceof ContractError)) throw err;
    process.stderr.write(`opencode-host-contract: ${err.message}\n`);
    process.exitCode = 1;
  }
}
