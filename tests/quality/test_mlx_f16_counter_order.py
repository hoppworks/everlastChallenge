"""Counter-order environment failure retains the cause and cleans its actual child."""
import importlib.util
import json
from pathlib import Path
import signal
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class CounterOrderTests(unittest.TestCase):
    def test_power_change_remains_primary_after_actual_owned_cleanup(self):
        spec = importlib.util.spec_from_file_location("f16_counter_order", ROOT / "artifacts/implementation/run-f16-runtime-counter-order.py")
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        control = runner.create_control()
        count = 0
        def power():
            nonlocal count
            count += 1
            return {"utc": "2026-10-20T00:00:00Z", "source": "AC Power" if count == 1 else "Battery Power",
                    "policy": "fixed", "batteryReadout": "bounded contract fixture"}
        control.power_snapshot = power
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            with self.assertRaisesRegex(ValueError, "power source/policy changed"):
                runner.base.owned([sys.executable, "-c", "import time; time.sleep(30)"], folder,
                    control, [], "contract", "a" * 64, timeout_seconds=5, sample_interval_seconds=.01)
            result = json.loads((folder / "owner-completion.json").read_text())
            self.assertTrue(result["processReaped"])
            self.assertTrue(result["terminationConfirmed"])
            self.assertFalse(result["forcedGroupCleanup"])
            self.assertEqual(result["exitCode"], -signal.SIGTERM)
            self.assertEqual(result["powerObservations"][-1]["phase"], "after")
            self.assertFalse(result["qualified"])
