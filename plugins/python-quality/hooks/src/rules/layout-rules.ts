/**
 * Test placement (#266, 20-tests): a test-bearing file must follow the
 * `test_*.py` / `*_test.py` convention, and a `test_*.py` needs a non-test
 * module beside it. `conftest.py` and `__init__.py` are exempt from both.
 */
import { lstatSync, readdirSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { ere, isPythonTestName, type PyFile } from "./py-file.ts";

const TEST_DEF = ere("^(async[[:space:]]+)?def[[:space:]]+test_[A-Za-z0-9_]*[[:space:]]*\\(");
/** grep's `\b` after the module name: the next character is not a word character. */
const TEST_IMPORT = ere("^(import|from)[[:space:]]+(pytest|unittest)(?![A-Za-z0-9_])");

function isModuleName(name: string): boolean {
  return (
    name.endsWith(".py") &&
    !isPythonTestName(name) &&
    name !== "conftest.py" &&
    name !== "__init__.py"
  );
}

function isRegular(path: string): boolean {
  try {
    return lstatSync(path).isFile();
  } catch {
    return false;
  }
}

/** `find DIR -maxdepth 1 -type f -name '*.py' ! -name <tests…>`: a regular module file, symlinks excluded. */
function hasModuleSibling(dir: string): boolean {
  let names: string[];
  try {
    names = readdirSync(dir);
  } catch {
    return false;
  }
  return names.some((name) => isModuleName(name) && isRegular(join(dir, name)));
}

export function testLayout(f: PyFile): string[] {
  const path = f.file.path;
  const base = basename(path);
  if (base === "conftest.py" || base === "__init__.py") return [];
  const errors: string[] = [];
  const marker = f.lines.some((line) => TEST_DEF.test(line) || TEST_IMPORT.test(line));
  if (marker && !isPythonTestName(base)) {
    errors.push(
      `Test-bearing file not named test_*.py or *_test.py: ${path} — rename to follow the colocated test convention`,
    );
  }
  if (
    base.startsWith("test_") &&
    base.endsWith(".py") &&
    !hasModuleSibling(dirname(f.file.absolute))
  ) {
    errors.push(
      `Test not co-located with a module: ${path} — colocate test_*.py next to the module it tests`,
    );
  }
  return errors;
}
