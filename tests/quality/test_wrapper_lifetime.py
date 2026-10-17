"""Actual upstream-wrapper ownership regression, offline and model-free."""
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]


class WrapperLifetimeTests(unittest.TestCase):
    def test_original_leaks_and_local_wrapper_reclaims_both_constructor_paths(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "vendor/whisper-rs/regression/check_lifetime.py")],
            cwd=ROOT, capture_output=True, text=True, timeout=90,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
