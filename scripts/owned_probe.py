"""Cleanup for probes launched in their own session; denial never means absence."""
import os
import signal
import subprocess
import time


def reap_owned(process, grace_seconds=10, observation_seconds=2):
    """Return explicit reap/absence evidence without masking the caller's error."""
    errors = []
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append({"operation": "terminateGroup", "errno": error.errno, "error": str(error)})
        try:
            process.wait(timeout=grace_seconds)
        except subprocess.TimeoutExpired:
            pass
        except OSError as error:
            errors.append({"operation": "wait", "errno": error.errno, "error": str(error)})
    def group_present():
        deadline = time.monotonic() + observation_seconds
        while True:
            state = None
            try:
                os.killpg(process.pid, 0)
                state = True
            except ProcessLookupError:
                return False
            except OSError as error:
                errors.append({"operation": "observeGroup", "errno": error.errno, "error": str(error)})
            if time.monotonic() >= deadline:
                return state
            time.sleep(min(.01, max(0, deadline - time.monotonic())))
    present = group_present()
    forced = present is True
    if forced:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append({"operation": "killGroup", "errno": error.errno, "error": str(error)})
        try:
            process.wait(timeout=grace_seconds)
        except subprocess.TimeoutExpired:
            pass
        except OSError as error:
            errors.append({"operation": "wait", "errno": error.errno, "error": str(error)})
        present = group_present()
    return {"exitCode": process.poll(), "processReaped": process.poll() is not None,
            "terminationConfirmed": present is False and process.poll() is not None,
            "forcedGroupCleanup": forced, "cleanupObservationErrors": errors}
