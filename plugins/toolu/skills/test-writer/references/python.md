# Python tests

## Write it well

- Write plain pytest functions with fixtures, and use `tmp_path` for files. If the repository uses `unittest`, follow it.
- Use real SQLite, temp dirs and local servers rather than `unittest.mock`. Fake time and randomness through injected parameters.
- Use `pytest.mark.parametrize` for a handful of spec examples. Use Hypothesis `@given` for invariants such as round trips, ordering or idempotence. It runs 100 examples by default. Use a `derandomize=True` profile on commit runs, and run larger, randomized runs nightly.
- Assert exact values and exception types: `pytest.raises(ValueError, match=...)`.

## Run it cheap

- Edit loop: `pytest path/to/test_x.py::test_name -x -q`.
- `pytest --testmon` (pytest-testmon) reruns only the tests whose executed code changed. It needs one full run first and does not track static files.
- `pytest -n <k>` (pytest-xdist): keep k small when other agents share the machine, since `-n auto` takes every core.
- To check that the tests kill mutants, run `mutmut` on the changed module only, never the whole package.
