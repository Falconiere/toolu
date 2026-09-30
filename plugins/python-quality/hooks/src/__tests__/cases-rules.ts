/** Golden cases (#266) ported from size.bats, docs.bats, suppression.bats and tests.bats. */
import { assignments, pyCase, wrote, type PyCase } from "./cases-types.ts";

const DOC = "missing a docstring";
const LONG = "Function too long";
const SUPPRESSED = "Forbidden suppression";
const MISNAMED = "not named test_*.py or *_test.py";
const LONELY = "Test not co-located with a module";

const limits = (python: object): Partial<PyCase> => ({ config: { lang: { python } } });

const SIZE: readonly PyCase[] = [
  pyCase("size: file at the default limit", wrote("ok.py", assignments(1, 400)), {
    expect: "silent",
    absent: ["exceeds"],
  }),
  pyCase("size: file one line over the default limit", wrote("big.py", assignments(1, 401)), {
    expect: "advisory",
    contains: ["exceeds 400-line limit"],
  }),
  pyCase(
    "size: project config lowers maxFileLines",
    wrote("big.py", assignments(1, 15)),
    { expect: "advisory", contains: ["exceeds 10-line limit"] },
    limits({ maxFileLines: 10 }),
  ),
  pyCase(
    "size: comments and blanks do not count",
    wrote(
      "commented.py",
      `${Array.from({ length: 20 }, (_, i) => `# comment ${String(i + 1)}\n`).join("")}\n\n${assignments(1, 4)}`,
    ),
    { expect: "silent", absent: ["exceeds"] },
    limits({ maxFileLines: 5 }),
  ),
  pyCase(
    "size: def at the default fn-length limit",
    wrote("ok.py", `def ok():\n${assignments(1, 48, "    ")}    return v1\n`),
    { expect: "advisory", absent: [LONG] },
  ),
  pyCase(
    "size: project config lowers maxFnLines",
    wrote("m.py", "def big():\n    a = 1\n    b = 2\n    c = 3\n    return a + b + c\n"),
    { expect: "advisory", contains: [LONG, "big:1"] },
    limits({ maxFnLines: 3 }),
  ),
  pyCase(
    "size: a decorator is not counted",
    wrote("m.py", "@staticmethod\ndef ok():\n    a = 1\n    b = 2\n    return a + b\n"),
    { expect: "advisory", absent: [LONG] },
    limits({ maxFnLines: 4 }),
  ),
  pyCase(
    "size: a method is measured to its own close",
    wrote(
      "m.py",
      "class Foo:\n    def short(self):\n        return 1\n\n    def long_method(self):\n        a = 1\n        b = 2\n        c = 3\n        return a + b + c\n",
    ),
    { expect: "advisory", contains: [LONG, "long_method"], absent: ["short:2"] },
    limits({ maxFnLines: 3 }),
  ),
  pyCase(
    "size: async def is measured",
    wrote("m.py", "async def big():\n    a = 1\n    b = 2\n    c = 3\n    return a + b + c\n"),
    { expect: "advisory", contains: [LONG] },
    limits({ maxFnLines: 3 }),
  ),
];

const DOCS: readonly PyCase[] = [
  pyCase("docs: top-level def without a docstring", wrote("m.py", "def undocumented():\n    return 1\n"), {
    expect: "advisory",
    contains: [DOC, "undocumented"],
    absent: ["QUALITY VIOLATION"],
  }),
  pyCase(
    "docs: top-level def with a docstring",
    wrote("m.py", 'def documented():\n    """Does a thing."""\n    return 1\n'),
    { expect: "silent" },
  ),
  pyCase(
    "docs: single-quoted docstring",
    wrote("m.py", "def documented():\n    '''Does a thing.'''\n    return 1\n"),
    { expect: "silent" },
  ),
  pyCase("docs: a _private def is exempt", wrote("m.py", "def _private():\n    return 1\n"), {
    expect: "silent",
  }),
  pyCase(
    "docs: an indented method is exempt",
    wrote("m.py", 'class Foo:\n    """A class."""\n\n    def method(self):\n        return 1\n'),
    { expect: "silent" },
  ),
  pyCase(
    "docs: a class without a docstring",
    wrote("m.py", "class Foo:\n    def method(self):\n        return 1\n"),
    { expect: "advisory", contains: [DOC, "Foo"] },
  ),
  pyCase(
    "docs: a multi-line signature is tracked to its colon",
    wrote("m.py", 'def multi_line(\n    a,\n    b,\n):\n    """Adds two numbers."""\n    return a + b\n'),
    { expect: "silent" },
  ),
  pyCase(
    "docs: a multi-line signature missing its docstring",
    wrote("m.py", "def multi_line(\n    a,\n    b,\n):\n    return a + b\n"),
    { expect: "advisory", contains: [DOC, "multi_line"] },
  ),
];

const SUPPRESSION: readonly PyCase[] = [
  pyCase("suppression: bare except:", wrote("m.py", "try:\n    pass\nexcept:\n    log_it()\n"), {
    expect: "advisory",
    contains: [SUPPRESSED, "bare except:"],
  }),
  pyCase("suppression: one-line except ...: pass", wrote("m.py", "try:\n    pass\nexcept ValueError: pass\n"), {
    expect: "advisory",
    contains: ["one-line except"],
  }),
  pyCase("suppression: blanket # noqa", wrote("m.py", "x = 1  # noqa\n"), {
    expect: "advisory",
    contains: ["blanket # noqa"],
  }),
  pyCase("suppression: blanket # type: ignore", wrote("m.py", 'x: int = "y"  # type: ignore\n'), {
    expect: "advisory",
    contains: ["blanket # type: ignore"],
  }),
  pyCase(
    "suppression: except SpecificError: with a body",
    wrote("m.py", "try:\n    pass\nexcept ValueError:\n    log_it()\n"),
    { expect: "silent" },
  ),
  pyCase("suppression: scoped # noqa: E501", wrote("m.py", "x = 1  # noqa: E501\n"), {
    expect: "silent",
  }),
  pyCase(
    "suppression: scoped # type: ignore[assignment]",
    wrote("m.py", 'x: int = "y"  # type: ignore[assignment]\n'),
    { expect: "silent" },
  ),
];

const MODULE = { "foo.py": "def module():\n    return 1\n" };
const TEST_BODY = "def test_something():\n    assert True\n";

const TESTS: readonly PyCase[] = [
  pyCase("tests: def test_ in a misnamed file", wrote("checks.py", TEST_BODY), {
    expect: "advisory",
    contains: [MISNAMED],
  }),
  pyCase(
    "tests: a pytest import in a misnamed file",
    wrote("checks.py", "import pytest\n\n\ndef check_thing():\n    assert True\n"),
    { expect: "advisory", contains: [MISNAMED] },
  ),
  pyCase(
    "tests: a unittest import in a misnamed file",
    wrote("checks.py", "from unittest import TestCase\n"),
    { expect: "advisory", contains: [MISNAMED] },
  ),
  pyCase(
    "tests: test_foo.py naming is accepted",
    [{ write: { ...MODULE, "test_foo.py": TEST_BODY }, file: "test_foo.py" }],
    { expect: "advisory", absent: ["not named test_"] },
  ),
  pyCase(
    "tests: foo_test.py naming is accepted",
    [{ write: { ...MODULE, "foo_test.py": TEST_BODY }, file: "foo_test.py" }],
    { expect: "advisory", absent: ["not named test_"] },
  ),
  pyCase("tests: test_*.py alone in its directory", wrote("lonely/test_orphan.py", TEST_BODY), {
    expect: "advisory",
    contains: [LONELY],
  }),
  pyCase(
    "tests: test_*.py next to its module",
    [
      {
        write: { "pkg/calc.py": "def add(a, b):\n    return a + b\n", "pkg/test_calc.py": "def test_add():\n    assert True\n" },
        file: "pkg/test_calc.py",
      },
    ],
    { expect: "advisory", absent: ["Test not co-located"] },
  ),
  pyCase(
    "tests: conftest.py importing pytest is exempt",
    wrote(
      "lonely/conftest.py",
      'import pytest\n\n\n@pytest.fixture\ndef thing():\n    """A fixture."""\n    return 1\n',
    ),
    { expect: "silent" },
  ),
  pyCase("tests: __init__.py is exempt", wrote("pkg/__init__.py", "import unittest\n"), {
    expect: "silent",
  }),
];

export const RULE_CASES: readonly PyCase[] = [...SIZE, ...DOCS, ...SUPPRESSION, ...TESTS];
