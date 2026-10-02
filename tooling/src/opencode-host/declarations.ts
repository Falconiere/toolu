/**
 * Read the pinned SDK's declared plugin hooks (#335) from the installed
 * `@opencode-ai/plugin` `dist/index.d.ts` with the TypeScript compiler API,
 * so the contract doc is checked against the declarations themselves.
 */
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join } from "node:path";
import ts from "typescript";
import { z } from "zod";
import { ContractError } from "./schema.ts";

type InstalledSdk = { dir: string; version: string };

const SdkManifest = z.looseObject({ name: z.literal("@opencode-ai/plugin"), version: z.string() });

/** The SDK installed for the adapter package (Bun's isolated link, else the root node_modules). */
export function installedSdk(adapterPackageJson: string, root: string): InstalledSdk {
  const candidates = [
    join(dirname(adapterPackageJson), "node_modules/@opencode-ai/plugin"),
    join(root, "node_modules/@opencode-ai/plugin"),
  ];
  const dir = candidates.find((candidate) => existsSync(join(candidate, "package.json")));
  if (dir === undefined) throw new ContractError("pinned SDK not installed: run bun install");
  const manifest = SdkManifest.parse(JSON.parse(readFileSync(join(dir, "package.json"), "utf8")));
  return { dir: realpathSync(dir), version: manifest.version };
}

function memberName(member: ts.TypeElement): string | null {
  const name = member.name;
  if (name === undefined) return null;
  if (ts.isIdentifier(name) || ts.isStringLiteral(name)) return name.text;
  return null;
}

/** Member names of the `Hooks` interface, in declaration order. */
export function declaredHooks(sdk: InstalledSdk): string[] {
  const path = join(sdk.dir, "dist/index.d.ts");
  const source = ts.createSourceFile(
    path,
    readFileSync(path, "utf8"),
    ts.ScriptTarget.Latest,
    true,
  );
  const hooks = source.statements.find(
    (statement): statement is ts.InterfaceDeclaration =>
      ts.isInterfaceDeclaration(statement) && statement.name.text === "Hooks",
  );
  if (hooks === undefined) throw new ContractError(`${path}: no Hooks interface`);
  return hooks.members.flatMap((member) => {
    const name = memberName(member);
    return name === null ? [] : [name];
  });
}
