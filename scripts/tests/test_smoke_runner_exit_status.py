"""Keep CI red when a requested SDK errors or produces no result."""

import contextlib
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "run_smoke_tests_parallel.py"
SPEC = importlib.util.spec_from_file_location("smoke_runner_exit_test", SCRIPT)
runner = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = runner
SPEC.loader.exec_module(runner)


class SmokeRunnerExitStatusTests(unittest.TestCase):
    def assert_cli_status(self, results, expected):
        output = io.StringIO()
        args = [str(SCRIPT), "--connectors", "stripe", "--sdks", "python"]
        with (
            patch.object(sys, "argv", args),
            patch.object(runner, "run_python_test_batch", return_value=results),
            contextlib.redirect_stdout(output),
            self.assertRaises(SystemExit) as result,
        ):
            runner.main()
        self.assertEqual(result.exception.code, expected)
        return output.getvalue()

    def test_sdk_setup_error_fails_cli_and_summary(self):
        result = runner.SDKResult("python", "stripe", "error", error="bindings missing")
        output = self.assert_cli_status([result], 1)
        self.assertIn("FAIL", output)

    def test_failed_flow_fails_cli(self):
        self.assert_cli_status([runner.SDKResult("python", "stripe", "failed")], 1)

    def test_missing_sdk_result_fails_cli(self):
        self.assert_cli_status([], 1)

    def test_successful_sdk_passes_cli(self):
        self.assert_cli_status([runner.SDKResult("python", "stripe", "passed")], 0)


if __name__ == "__main__":
    unittest.main()
