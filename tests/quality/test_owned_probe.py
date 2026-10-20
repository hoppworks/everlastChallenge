"""Owned process-group cleanup must distinguish denied observations from absence."""
import importlib.util
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


class OwnedProbeTests(unittest.TestCase):
    def test_unresponsive_owned_probe_requires_forced_cleanup_and_actual_absence(self):
        spec = importlib.util.spec_from_file_location("owned_probe_stubborn", ROOT / "scripts/owned_probe.py")
        owner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(owner)
        process = subprocess.Popen([sys.executable, "-c",
            "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print('ready',flush=True); time.sleep(30)"],
            start_new_session=True, stdout=subprocess.PIPE)
        try:
            self.assertTrue(select.select([process.stdout], [], [], 5)[0])
            self.assertEqual(process.stdout.readline(), b"ready\n")
            result = owner.reap_owned(process, grace_seconds=.1, observation_seconds=.05)
            self.assertTrue(result["processReaped"])
            self.assertTrue(result["terminationConfirmed"])
            self.assertTrue(result["forcedGroupCleanup"])
            self.assertEqual(result["exitCode"], -signal.SIGKILL)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            process.stdout.close()

    def test_live_owned_probe_is_terminated_reaped_and_confirmed_absent(self):
        spec = importlib.util.spec_from_file_location("owned_probe_live", ROOT / "scripts/owned_probe.py")
        owner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(owner)
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], start_new_session=True)
        try:
            result = owner.reap_owned(process, grace_seconds=2, observation_seconds=.1)
            self.assertTrue(result["processReaped"])
            self.assertTrue(result["terminationConfirmed"])
            self.assertEqual(result["exitCode"], -signal.SIGTERM)
            self.assertFalse(result["forcedGroupCleanup"])
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)

    def test_transient_denied_group_observation_keeps_the_actual_later_absence_proof(self):
        spec = importlib.util.spec_from_file_location("owned_probe", ROOT / "scripts/owned_probe.py")
        owner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(owner)
        process = subprocess.Popen([sys.executable, "-c", "pass"], start_new_session=True)
        self.assertEqual(process.wait(timeout=5), 0)
        actual_killpg = os.killpg
        calls = []
        def observe(group, signum):
            calls.append((group, signum))
            self.assertEqual(group, process.pid)
            if len(calls) == 1:
                raise PermissionError(1, "Operation not permitted")
            return actual_killpg(group, signum)
        with mock.patch.object(owner.os, "killpg", side_effect=observe):
            result = owner.reap_owned(process, grace_seconds=.1, observation_seconds=.1)
        self.assertTrue(result["processReaped"])
        self.assertTrue(result["terminationConfirmed"])
        self.assertFalse(result["forcedGroupCleanup"])
        self.assertEqual(result["exitCode"], 0)
        self.assertEqual(result["cleanupObservationErrors"][0]["errno"], 1)
        self.assertTrue(all(signum == 0 for _, signum in calls), "completed absent groups need no signal")

    def test_persistent_permission_denial_is_recorded_as_unconfirmed(self):
        spec = importlib.util.spec_from_file_location("owned_probe_denied", ROOT / "scripts/owned_probe.py")
        owner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(owner)
        process = subprocess.Popen([sys.executable, "-c", "pass"], start_new_session=True)
        process.wait(timeout=5)
        with mock.patch.object(owner.os, "killpg", side_effect=PermissionError(1, "Operation not permitted")):
            result = owner.reap_owned(process, grace_seconds=.1, observation_seconds=.02)
        self.assertTrue(result["processReaped"])
        self.assertFalse(result["terminationConfirmed"])
        self.assertTrue(result["cleanupObservationErrors"])
