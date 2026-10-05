/**
 * Static consistency of `.github/ci-paths.json` and the workflows (#458): no
 * workflow-level path filter where a required check is reported, aggregates
 * need exactly `changes` plus their gated jobs, every gated job reads its own
 * group, no job reads a group the data file does not map, and every glob
 * matches a tracked file.
 */
import { CHANGED, type CiPaths, type CiWorkflow, matchesAny } from "./config.ts";
import {
  conditionOf,
  groupsRead,
  type Job,
  needsOf,
  pathFilteredTriggers,
  type WorkflowFile,
} from "./workflow.ts";

const CHANGES = "changes";

function sameSet(left: readonly string[], right: readonly string[]): boolean {
  const a = [...new Set(left)].toSorted();
  const b = [...new Set(right)].toSorted();
  return a.length === b.length && a.every((item, index) => item === b[index]);
}

function checkGatedJob(
  file: string,
  id: string,
  group: string,
  job: Job | undefined,
  config: CiPaths,
): string[] {
  if (job === undefined) return [`${file}: gated job ${id} does not exist`];
  const problems: string[] = [];
  if (group !== CHANGED && config.groups[group] === undefined) {
    problems.push(`${file}: job ${id} is mapped to undefined group ${group}`);
  }
  if (!needsOf(job).includes(CHANGES)) problems.push(`${file}: job ${id} does not need ${CHANGES}`);
  const gate = new RegExp(String.raw`needs\.changes\.outputs\.${group} (?:== 'true'|!= 'false')`);
  if (!gate.test(conditionOf(job))) {
    problems.push(`${file}: job ${id} is not gated on needs.changes.outputs.${group}`);
  }
  return problems;
}

function checkAggregate(file: string, entry: CiWorkflow, workflow: WorkflowFile): string[] {
  if (entry.aggregate === null) return [];
  const job = workflow.jobs[entry.aggregate];
  if (job === undefined) return [`${file}: aggregate ${entry.aggregate} does not exist`];
  const problems: string[] = [];
  const expected = [CHANGES, ...Object.keys(entry.jobs)];
  if (!sameSet(needsOf(job), expected)) {
    problems.push(
      `${file}: aggregate ${entry.aggregate} needs [${needsOf(job).join(", ")}], expected [${expected.join(", ")}]`,
    );
  }
  if (!conditionOf(job).includes("always()")) {
    problems.push(`${file}: aggregate ${entry.aggregate} must run with if: always()`);
  }
  return problems;
}

function checkChangesJob(file: string, entry: CiWorkflow, workflow: WorkflowFile): string[] {
  const job = workflow.jobs[CHANGES];
  if (job === undefined) return [`${file}: has no ${CHANGES} job`];
  const outputs = Object.keys(job.outputs ?? {});
  return [...new Set(Object.values(entry.jobs))]
    .filter((group) => !outputs.includes(group))
    .map((group) => `${file}: ${CHANGES} job does not output ${group}`);
}

function checkWorkflow(entry: CiWorkflow, workflow: WorkflowFile, config: CiPaths): string[] {
  const { file } = workflow;
  const problems = pathFilteredTriggers(workflow).map(
    (trigger) => `${file}: reports a required check but filters ${trigger} by paths`,
  );
  for (const [id, group] of Object.entries(entry.jobs)) {
    problems.push(...checkGatedJob(file, id, group, workflow.jobs[id], config));
  }
  for (const name of entry.required) {
    const found = Object.entries(workflow.jobs).some(([id, job]) => (job.name ?? id) === name);
    if (!found) problems.push(`${file}: required check ${name} is not a job`);
  }
  return [
    ...problems,
    ...checkAggregate(file, entry, workflow),
    ...checkChangesJob(file, entry, workflow),
  ];
}

function checkUnmapped(workflow: WorkflowFile, entry: CiWorkflow | undefined): string[] {
  return Object.entries(workflow.jobs).flatMap(([id, job]) => {
    if (groupsRead(job).length === 0 || entry?.jobs[id] !== undefined) return [];
    return [
      `${workflow.file}: job ${id} reads needs.changes.outputs but has no group in the data file`,
    ];
  });
}

function checkGlobs(config: CiPaths, tracked: readonly string[]): string[] {
  const globs = [
    ...Object.values(config.groups).flat(),
    ...config.runEverything,
    ...config.releaseOnly.paths,
  ];
  return [...new Set(globs)]
    .filter((glob) => !tracked.some((path) => matchesAny([glob], path)))
    .map((glob) => `ci-paths.json: ${glob} matches no tracked file`);
}

/** Every violation, one line each; empty when consistent. */
export function checkCiPaths(
  config: CiPaths,
  workflows: readonly WorkflowFile[],
  tracked: readonly string[],
): string[] {
  const problems: string[] = [];
  for (const file of Object.keys(config.workflows)) {
    if (!workflows.some((workflow) => workflow.file === file)) {
      problems.push(`ci-paths.json: workflow ${file} does not exist`);
    }
  }
  for (const workflow of workflows) {
    const entry = config.workflows[workflow.file];
    if (entry !== undefined) problems.push(...checkWorkflow(entry, workflow, config));
    problems.push(...checkUnmapped(workflow, entry));
  }
  return [...problems, ...checkGlobs(config, tracked)];
}
