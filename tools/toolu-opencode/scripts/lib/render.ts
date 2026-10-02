/** Kind-specific OpenCode Markdown frontmatter mapping. */
import type { ArtifactKind } from "./constants.ts";
import { parseFrontmatter, serializeFrontmatter, type FrontmatterRecord } from "./frontmatter.ts";
import { rewriteBody, type SurfaceReferences } from "./rewrite.ts";

function nonemptyString(value: unknown, label: string, sourcePath: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${sourcePath}: ${label} must be a nonempty string`);
  }
  return value.trim();
}

function optionalString(value: unknown, label: string, sourcePath: string): string | undefined {
  if (value === undefined) return undefined;
  return nonemptyString(value, label, sourcePath);
}

function stringMap(value: unknown, label: string, sourcePath: string): Record<string, string> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${sourcePath}: ${label} must be a string map`);
  }
  const out: Record<string, string> = {};
  for (const [key, item] of Object.entries(value)) {
    out[key] = nonemptyString(item, `${label}.${key}`, sourcePath);
  }
  return out;
}

const CLAUDE_TOOL_TO_PERMISSION: Record<string, string> = {
  Read: "read",
  Grep: "grep",
  Glob: "glob",
  Bash: "bash",
  Edit: "edit",
  Write: "edit",
  WebSearch: "websearch",
  WebFetch: "webfetch",
};

function agentPermission(tools: unknown, sourcePath: string): Record<string, string> {
  const names =
    typeof tools === "string"
      ? tools.split(",").map((name) => name.trim())
      : Array.isArray(tools)
        ? tools
        : null;
  if (!names || !names.length || names.some((name) => typeof name !== "string")) {
    throw new Error(`${sourcePath}: agent tools must be a nonempty list`);
  }
  const out: Record<string, string> = { "*": "deny" };
  for (const name of names) {
    const permission = CLAUDE_TOOL_TO_PERMISSION[name as string];
    if (!permission) throw new Error(`${sourcePath}: unsupported Claude agent tool: ${name}`);
    out[permission] = "allow";
  }
  return out;
}

function commandDescription(
  frontmatter: FrontmatterRecord,
  body: string,
  sourcePath: string,
): string {
  if (frontmatter.description !== undefined) {
    return nonemptyString(frontmatter.description, "description", sourcePath);
  }
  const heading = /^#\s+(.+)$/m.exec(body);
  if (!heading) throw new Error(`${sourcePath}: command needs a description or heading`);
  return (heading[1] ?? "").trim();
}

function mappedFrontmatter(
  kind: ArtifactKind,
  surfaceId: string,
  frontmatter: FrontmatterRecord,
  body: string,
  sourcePath: string,
): FrontmatterRecord {
  if (kind === "skill") {
    const description = nonemptyString(frontmatter.description, "skill description", sourcePath);
    if (description.length > 1024)
      throw new Error(`${sourcePath}: skill description exceeds 1024 characters`);
    const out: FrontmatterRecord = { name: surfaceId, description };
    for (const key of ["license", "compatibility"]) {
      const value = optionalString(frontmatter[key], key, sourcePath);
      if (value !== undefined) out[key] = value;
    }
    if (frontmatter.metadata !== undefined) {
      out.metadata = stringMap(frontmatter.metadata, "metadata", sourcePath);
    }
    return out;
  }
  if (kind === "agent") {
    const out: FrontmatterRecord = {
      description: nonemptyString(frontmatter.description, "agent description", sourcePath),
      mode: "subagent",
    };
    if (frontmatter.tools !== undefined) {
      if (frontmatter.permission !== undefined) {
        throw new Error(`${sourcePath}: agent tools and permission cannot both be specified`);
      }
      out.permission = agentPermission(frontmatter.tools, sourcePath);
    } else if (frontmatter.permission !== undefined) {
      out.permission = frontmatter.permission;
    }
    const model = optionalString(frontmatter.model, "model", sourcePath);
    if (model?.includes("/")) out.model = model;
    for (const key of ["temperature", "steps", "hidden", "color"]) {
      if (frontmatter[key] !== undefined) out[key] = frontmatter[key];
    }
    return out;
  }
  const out: FrontmatterRecord = { description: commandDescription(frontmatter, body, sourcePath) };
  const agent = optionalString(frontmatter.agent, "agent", sourcePath);
  if (agent !== undefined) out.agent = agent;
  const model = optionalString(frontmatter.model, "model", sourcePath);
  if (model?.includes("/")) out.model = model;
  if (typeof frontmatter.subtask === "boolean") out.subtask = frontmatter.subtask;
  return out;
}

export function renderMarkdown(
  kind: ArtifactKind,
  surfaceId: string,
  sourcePath: string,
  text: string,
  references: SurfaceReferences,
  plugin: string,
  commandSkillId?: string,
): {
  content: string;
  strippedKeys: string[];
  rewriteNotes: ReturnType<typeof rewriteBody>["notes"];
} {
  let parsed: ReturnType<typeof parseFrontmatter>;
  try {
    parsed = parseFrontmatter(text);
  } catch (error) {
    throw new Error(`${sourcePath}: invalid YAML frontmatter`, { cause: error });
  }
  const body = commandSkillId
    ? `Load the \`${commandSkillId}\` skill with the native skill tool and follow its instructions. Pass $ARGUMENTS as task context.\n`
    : parsed.body;
  const rewritten = rewriteBody(
    body,
    references,
    kind === "skill" ? { plugin, skillId: surfaceId } : { plugin },
  );
  const frontmatter = mappedFrontmatter(
    kind,
    surfaceId,
    parsed.frontmatter,
    parsed.body,
    sourcePath,
  );
  return {
    content: `${serializeFrontmatter(frontmatter)}${rewritten.body}`,
    strippedKeys: parsed.strippedKeys,
    rewriteNotes: rewritten.notes,
  };
}
