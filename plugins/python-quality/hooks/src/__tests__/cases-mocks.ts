/**
 * Golden cases (#266) ported from no-mocks.bats, plus the ast-grep failure
 * stages (a stub `ast-grep` in front of the real one) and ast-grep absent.
 */
import { pathWithout, stubAstGrep } from "./cases-path.ts";
import { pyCase, wrote, type PyCase, type Step } from "./cases-types.ts";

const IMPORT = "no-mocks: mock import";
const PARAM = "no-mocks: mocker/monkeypatch fixture parameter";
const FAILED = "ast-grep failed while scanning";

/** `printf '<import>\n\n\ndef test_it():\n    """Test it."""\n    assert <name>\n' > test_a.py`. */
const withImport = (line: string, name: string) =>
  `${line}\n\n\ndef test_it():\n    """Test it."""\n    assert ${name}\n`;

const withParam = (param: string) =>
  `def test_it(${param}):\n    """Test it."""\n    assert ${param}\n`;

/** A colocated, documented, mock-free test: only the no-mocks rule can speak. */
const colocated = (body: string): Step[] => [
  { write: { "pkg/mod.py": '"""Mod."""\n', "pkg/test_mod.py": body }, file: "pkg/test_mod.py" },
];
const CLEAN_TEST = 'def test_add():\n    """Add two numbers."""\n    assert 1 + 1 == 2\n';

const BATS: readonly PyCase[] = [
  pyCase(
    "no-mocks: from unittest import mock",
    wrote("test_a.py", withImport("from unittest import mock", "mock")),
    {
      expect: "advisory",
      contains: [IMPORT],
    },
  ),
  pyCase(
    "no-mocks: import unittest.mock",
    wrote("test_a.py", withImport("import unittest.mock", "unittest.mock")),
    {
      expect: "advisory",
      contains: [IMPORT],
    },
  ),
  pyCase(
    "no-mocks: from mock import Mock",
    wrote("test_a.py", withImport("from mock import Mock", "Mock")),
    {
      expect: "advisory",
      contains: [IMPORT],
    },
  ),
  pyCase(
    "no-mocks: import pytest_mock",
    wrote("test_a.py", withImport("import pytest_mock", "pytest_mock")),
    {
      expect: "advisory",
      contains: [IMPORT],
    },
  ),
  pyCase(
    "no-mocks: from unittest.mock import MagicMock, patch",
    wrote(
      "test_a.py",
      withImport("from unittest.mock import MagicMock, patch", "MagicMock and patch"),
    ),
    { expect: "advisory", contains: [IMPORT] },
  ),
  pyCase("no-mocks: mocker fixture parameter", wrote("test_a.py", withParam("mocker")), {
    expect: "advisory",
    contains: [PARAM],
  }),
  pyCase("no-mocks: monkeypatch fixture parameter", wrote("test_a.py", withParam("monkeypatch")), {
    expect: "advisory",
    contains: [PARAM],
  }),
  pyCase(
    "no-mocks: a mock import in a non-test file",
    wrote("helper.py", 'import mock\n\n\ndef helper():\n    """Use mock."""\n    return mock\n'),
    { expect: "silent", absent: ["no-mocks"] },
  ),
  pyCase(
    "no-mocks: lang.python.noMocks false",
    wrote("test_a.py", withImport("from unittest import mock", "mock")),
    { expect: "advisory", absent: ["no-mocks"] },
    { config: { lang: { python: { noMocks: false } } } },
  ),
  pyCase("no-mocks: a real-fixture test passes", wrote("test_a.py", CLEAN_TEST), {
    expect: "advisory",
    absent: ["no-mocks"],
  }),
];

const SCAN: readonly PyCase[] = [
  pyCase(
    "no-mocks: ast-grep absent skips the rule",
    colocated(withImport("import mock", "mock")),
    {
      expect: "silent",
      absent: ["no-mocks"],
    },
    { env: pathWithout("ast-grep") },
  ),
  pyCase(
    "no-mocks: ast-grep exit 2 with stderr",
    colocated(CLEAN_TEST),
    {
      expect: "advisory",
      contains: [FAILED, "ast-grep exit 2: boom"],
    },
    { env: stubAstGrep("echo boom >&2\nexit 2") },
  ),
  pyCase(
    "no-mocks: ast-grep stderr with exit 0",
    colocated(CLEAN_TEST),
    {
      expect: "advisory",
      contains: [FAILED, "ast-grep exit 0: warn"],
    },
    { env: stubAstGrep("echo '[]'\necho warn >&2") },
  ),
  pyCase(
    "no-mocks: ast-grep exit 0 with empty output",
    colocated(CLEAN_TEST),
    {
      expect: "advisory",
      contains: [FAILED, "exited 0 with empty output"],
    },
    { env: stubAstGrep("exit 0") },
  ),
  pyCase(
    "no-mocks: ast-grep exit 0 with non-JSON output",
    colocated(CLEAN_TEST),
    {
      expect: "advisory",
      contains: [FAILED, "did not parse as the documented JSON array"],
    },
    { env: stubAstGrep("echo 'not json'") },
  ),
  pyCase(
    "no-mocks: ast-grep JSON that is not an array of matches",
    colocated(CLEAN_TEST),
    {
      expect: "advisory",
      contains: [FAILED, "did not parse"],
    },
    { env: stubAstGrep("echo '{\"a\":1}'") },
  ),
];

export const MOCK_CASES: readonly PyCase[] = [...BATS, ...SCAN];
