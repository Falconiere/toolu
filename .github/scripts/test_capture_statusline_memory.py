"""The memory observer must stop and reap its own real tracer process."""

import signal
import subprocess
import sys
import unittest

from capture_statusline_memory import stop


class CaptureCleanupTest(unittest.TestCase):
    def test_normal_and_term_ignoring_processes_are_stopped_and_reaped(self):
        for ignore_term, expected in [(False, -signal.SIGTERM), (True, -signal.SIGKILL)]:
            with self.subTest(ignore_term=ignore_term):
                script = "import signal,sys; "
                if ignore_term:
                    script += "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                script += 'print("READY", flush=True); sys.stdin.read()'
                process = subprocess.Popen(
                    [sys.executable, "-c", script],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                )
                try:
                    self.assertEqual(process.stdout.readline(), b"READY\n")
                    stop(process)
                    self.assertEqual(process.returncode, expected)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=5)
                    process.stdin.close()
                    process.stdout.close()


class CaptureCountTest(unittest.TestCase):
    def test_exactly_five_captures_are_required_under_optimized_python(self):
        script = '''
import ast
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text()
module = ast.parse(source)
main = next(node for node in module.body if isinstance(node, ast.FunctionDef) and node.name == "main")
report = next(i for i, node in enumerate(main.body)
              if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id == "report" for target in node.targets))
guard = ast.Module(body=[main.body[report - 1]], type_ignores=[])
snapshots = dict.fromkeys(range(int(sys.argv[2])))
exec(compile(guard, sys.argv[1], "exec"))
'''
        from pathlib import Path

        source = Path(__file__).with_name("capture_statusline_memory.py")
        for count in (0, 4, 5, 6):
            with self.subTest(count=count):
                result = subprocess.run(
                    [sys.executable, "-O", "-c", script, str(source), str(count)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(result.returncode == 0, count == 5, result.stderr)


if __name__ == "__main__":
    unittest.main()
