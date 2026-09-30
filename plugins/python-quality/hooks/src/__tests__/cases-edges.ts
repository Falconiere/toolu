/**
 * Golden cases (#266) for the grep/awk quirks the port must keep: output
 * caps, line endings, tabs, regex boundaries, the docs scanner's state
 * machine, the indent-based function span and `find -type f`.
 */
import { mkdirSync, symlinkSync } from "node:fs";
import { dirname } from "node:path";
import { pyCase, wrote, type PyCase } from "./cases-types.ts";

const repeat = (n: number, line: (i: number) => string) =>
  Array.from({ length: n }, (_, i) => line(i + 1)).join("");

const limits = (python: object): Partial<PyCase> => ({ config: { lang: { python } } });

const CAPS: readonly PyCase[] = [
  pyCase(
    "edges: more than five suppressions list the first five",
    wrote(
      "m.py",
      repeat(7, (i) => `x${String(i)} = 1  # noqa\n`),
    ),
    {
      expect: "advisory",
      contains: ["5: blanket # noqa"],
      absent: ["6: blanket"],
    },
  ),
  pyCase(
    "edges: more than five mock imports list the first five",
    wrote(
      "test_a.py",
      `${repeat(7, () => "import mock\n")}\n\ndef test_it():\n    """T."""\n    assert mock\n`,
    ),
    { expect: "advisory", contains: ["5: import mock"], absent: ["6: import mock"] },
  ),
  pyCase(
    "edges: more than three undocumented defs list the first three",
    wrote(
      "m.py",
      repeat(5, (i) => `def f${String(i)}():\n    return ${String(i)}\n\n\n`),
    ),
    { expect: "advisory", contains: ["f3"], absent: ["f4"] },
  ),
  pyCase(
    "edges: every long function is listed",
    wrote(
      "m.py",
      repeat(3, (i) => `def g${String(i)}():\n    """D."""\n    a = 1\n    return a\n\n\n`),
    ),
    { expect: "advisory", contains: ["g1:1", "g2:7", "g3:13"] },
    limits({ maxFnLines: 2 }),
  ),
];

const LINES: readonly PyCase[] = [
  pyCase("edges: CRLF bare except", wrote("m.py", "try:\r\n    pass\r\nexcept:\r\n    pass\r\n"), {
    expect: "advisory",
    contains: ["bare except:"],
  }),
  // A CRLF signature ends in `\r`, not `:`, so the docs scanner never checks a body.
  pyCase(
    "edges: CRLF docstring and signature",
    wrote("m.py", 'def a():\r\n    """Doc."""\r\n    return 1\r\ndef b():\r\n    return 2\r\n'),
    { expect: "silent" },
  ),
  pyCase("edges: no final newline", wrote("m.py", "try:\n    pass\nexcept:\n    pass"), {
    expect: "advisory",
    contains: ["bare except:"],
  }),
  pyCase("edges: empty file", wrote("m.py", ""), { expect: "silent" }),
  pyCase(
    "edges: one-line except pass with a comment",
    wrote("m.py", "try:\n    x()\nexcept ValueError: pass  # ignored\n"),
    { expect: "advisory", contains: ["3: one-line except"] },
  ),
  pyCase(
    "edges: noqa and type-ignore spellings",
    wrote(
      "m.py",
      "a = 1  # noqa:E501\nb = 2  # noqa : x\nc = 3  #noqa\nd = 4  # NOQA\ne = 5  # type:ignore\nf = 6  # type: ignore [x]\ng = 7\t#\tnoqa\n",
    ),
    {
      expect: "advisory",
      contains: [
        "3: blanket # noqa",
        "5: blanket # type",
        "6: blanket # type",
        "7: blanket # noqa",
      ],
      absent: ["1: ", "2: ", "4: "],
    },
  ),
  pyCase("edges: tab-indented except", wrote("m.py", "try:\n\tpass\n\texcept\t:\t# x\n"), {
    expect: "advisory",
    contains: ["3: bare except:"],
  }),
];

const MOCK_SHAPES: readonly PyCase[] = [
  pyCase(
    "edges: mocker in a multi-line and a nested def",
    wrote(
      "test_a.py",
      'def test_outer(\n    mocker,\n):\n    """Outer."""\n    def inner(monkeypatch):\n        return monkeypatch\n    assert inner and mocker\n',
    ),
    { expect: "advisory", contains: ["1: def test_outer(", "5:     def inner(monkeypatch):"] },
  ),
  pyCase(
    "edges: a tab in a mock hit line",
    wrote("test_a.py", 'def test_it(mocker):\t# fixture\n    """T."""\n    assert mocker\n'),
    { expect: "advisory", contains: ["1: def test_it(mocker):"], absent: ["# fixture"] },
  ),
  pyCase(
    "edges: conftest.py with a mock import is not scanned",
    [
      {
        write: { "pkg/mod.py": '"""M."""\n', "pkg/conftest.py": "from unittest import mock\n" },
        file: "pkg/conftest.py",
      },
    ],
    { expect: "silent" },
  ),
];

const DOC_SHAPES: readonly PyCase[] = [
  pyCase(
    "edges: docstring prefixes",
    wrote(
      "m.py",
      'def f():\n    r"""Raw."""\n\n\ndef g():\n    b"""Bytes."""\n\n\ndef h():\n    F\'\'\'Fmt.\'\'\'\n',
    ),
    { expect: "advisory", contains: ["5: g"], absent: ["1: f", "9: h"] },
  ),
  pyCase(
    "edges: a one-line def swallows the next def",
    wrote("m.py", "def one(): return 1\ndef two():\n    return 2\n"),
    { expect: "advisory", contains: ["1: one"], absent: ["two"] },
  ),
  pyCase(
    "edges: a comment after the signature is not a docstring",
    wrote("m.py", 'async def a():  # note\n    # first\n    """Late."""\n'),
    { expect: "advisory", contains: ["1: a"] },
  ),
];

const SPANS: readonly PyCase[] = [
  pyCase(
    "edges: a nested def counts toward its outer",
    wrote(
      "m.py",
      'def outer():\n    """Doc."""\n    def inner():\n        return 1\n    return inner\n',
    ),
    { expect: "advisory", contains: ["outer:1 (5 lines)"], absent: ["inner:"] },
    limits({ maxFnLines: 4 }),
  ),
  pyCase(
    "edges: comments inside a def are not counted",
    wrote("m.py", 'def f():\n    """D."""\n    # a\n    # b\n\n    return 1\n'),
    { expect: "silent" },
    limits({ maxFnLines: 3 }),
  ),
];

const LAYOUT: readonly PyCase[] = [
  pyCase(
    "edges: a symlinked sibling is not a module",
    wrote("lonely/test_x.py", "def test_x():\n    assert True\n"),
    { expect: "advisory", contains: ["Test not co-located"] },
    {
      setup: (sb) => {
        sb.write("other/mod.py", "X = 1\n");
        mkdirSync(dirname(sb.path("lonely/mod.py")), { recursive: true });
        symlinkSync(sb.path("other/mod.py"), sb.path("lonely/mod.py"));
      },
    },
  ),
  pyCase(
    "edges: pytest-prefixed import names are not test markers",
    wrote("helper.py", '"""H."""\nimport pytest_asyncio\nimport unittestx\n'),
    { expect: "silent" },
  ),
  pyCase(
    "edges: test marker variants in a misnamed file",
    wrote("checks.py", "async def test_x ():\n    assert True\n"),
    { expect: "advisory", contains: ["not named test_"] },
  ),
  pyCase(
    "edges: an indented test method is not a marker",
    wrote("checks.py", "class TestA:\n    def test_x(self):\n        assert True\n"),
    { expect: "advisory", absent: ["not named test_"], contains: ["TestA"] },
  ),
];

export const EDGE_CASES: readonly PyCase[] = [
  ...CAPS,
  ...LINES,
  ...MOCK_SHAPES,
  ...DOC_SHAPES,
  ...SPANS,
  ...LAYOUT,
];
