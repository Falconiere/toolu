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


if __name__ == "__main__":
    unittest.main()
