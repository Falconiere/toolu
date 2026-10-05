/** GitHub workflow files as the CI path checks read them (#458). */
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";

const Needs = z.union([z.string(), z.array(z.string())]);

const JobSchema = z.looseObject({
  name: z.string().optional(),
  needs: Needs.optional(),
  if: z.union([z.string(), z.boolean()]).optional(),
  outputs: z.record(z.string(), z.string()).optional(),
});

const WorkflowFileSchema = z.looseObject({
  on: z.union([z.string(), z.array(z.string()), z.record(z.string(), z.unknown())]),
  jobs: z.record(z.string(), JobSchema),
});

export type Job = z.infer<typeof JobSchema>;
export type WorkflowFile = z.infer<typeof WorkflowFileSchema> & { file: string };

/** Every `*.yml`/`*.yaml` under `dir`, parsed; a file that does not parse is an error line. */
export function readWorkflows(dir: string): { workflows: WorkflowFile[]; errors: string[] } {
  const workflows: WorkflowFile[] = [];
  const errors: string[] = [];
  const files = readdirSync(dir)
    .filter((file) => /\.ya?ml$/.test(file))
    .toSorted();
  for (const file of files) {
    let doc: unknown;
    try {
      doc = Bun.YAML.parse(readFileSync(join(dir, file), "utf8"));
    } catch (error) {
      errors.push(`${file}: not valid YAML: ${String(error)}`);
      continue;
    }
    const parsed = WorkflowFileSchema.safeParse(doc);
    if (parsed.success) workflows.push({ ...parsed.data, file });
    else errors.push(`${file}: not a workflow: ${z.prettifyError(parsed.error)}`);
  }
  return { workflows, errors };
}

/** A job's `needs` as a list. */
export function needsOf(job: Job): string[] {
  if (job.needs === undefined) return [];
  return typeof job.needs === "string" ? [job.needs] : job.needs;
}

/** A job's `if` as text (`""` when absent). */
export function conditionOf(job: Job): string {
  return job.if === undefined ? "" : String(job.if);
}

/** The groups a job's `if` reads from `needs.changes.outputs`. */
export function groupsRead(job: Job): string[] {
  return [...conditionOf(job).matchAll(/needs\.changes\.outputs\.([A-Za-z0-9_-]+)/g)].flatMap(
    (match) => (match[1] === undefined ? [] : [match[1]]),
  );
}

/** Triggers that carry a workflow-level `paths` or `paths-ignore` filter. */
export function pathFilteredTriggers(workflow: WorkflowFile): string[] {
  if (typeof workflow.on === "string" || Array.isArray(workflow.on)) return [];
  return Object.entries(workflow.on).flatMap(([trigger, config]) => {
    const filters = z.looseObject({ paths: z.unknown(), "paths-ignore": z.unknown() }).partial();
    const parsed = filters.safeParse(config);
    if (!parsed.success) return [];
    const has = parsed.data.paths !== undefined || parsed.data["paths-ignore"] !== undefined;
    return has ? [trigger] : [];
  });
}
