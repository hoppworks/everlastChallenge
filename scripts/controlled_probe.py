"""Owned, power-controlled benchmark execution without historical artifact imports.

Extracted from the retained run-f16-runtime-central.py::owned,
run-reference-encoder-power-control.py power contract, and
run-f16-runtime-quality.py::write. Historical runners/evidence stay unchanged;
this module makes their process-cleanup regression runnable from a clean clone.
It launches only the caller's explicit command, never a model campaign on import.
"""
import datetime
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("controlled_owned_cleanup", Path(__file__).with_name("owned_probe.py"))
owner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(owner)


def write(folder, name, value):
    fd = os.open(folder / name, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2, allow_nan=False)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def power_snapshot():
    def read(arguments):
        return subprocess.run(["/usr/bin/pmset", "-g", arguments], check=True,
                              capture_output=True, text=True, timeout=5).stdout
    battery = read("batt")
    policy = read("custom")
    match = re.match(r"Now drawing from '(AC Power|Battery Power)'", battery)
    if match is None or len(policy) > 16 * 1024 or not policy.strip():
        raise ValueError("unrecognized power source/policy")
    return {"utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "source": match.group(1), "policy": policy, "batteryReadout": battery}

def require_same_power(rows, require_pair=True):
    if (len(rows) < (2 if require_pair else 1)
            or rows[0].get("source") not in ("AC Power", "Battery Power")):
        raise ValueError("missing/invalid initial power observation")
    first = (rows[0]["source"], rows[0].get("policy"))
    if not isinstance(first[1], str) or not first[1].strip():
        raise ValueError("missing power policy")
    if any((row.get("source"), row.get("policy")) != first for row in rows):
        raise ValueError("power source/policy changed; retain the run, no qualification/retry")

def require_arm_power(observations, label):
    rows = [row for row in observations if row.get("arm") == label]
    require_same_power(rows)
    if (rows[0].get("phase") != "before" or rows[-1].get("phase") != "after"
            or any(row.get("phase") != "sample" for row in rows[1:-1])):
        raise ValueError("each arm requires exactly one before and post-reap after observation")
    return {"before": rows[0]["utc"], "after": rows[-1]["utc"],
            "sampleCount": len(rows), "duringCount": len(rows) - 2}


def create_control():
    """Each bounded run owns independent cancellation and child-process state."""
    control = SimpleNamespace(STOPPED=False, ACTIVE=None, reap=owner.reap_owned,
                              power_snapshot=power_snapshot,
                              require_same_power=require_same_power,
                              require_arm_power=require_arm_power)
    def request_stop(_signal, _frame):
        control.STOPPED = True
    control.stop = request_stop
    return control


def owned(command, folder, control, observations, label, revision,
          timeout_seconds=6000, sample_interval_seconds=5, launcher=subprocess.Popen):
    if control.STOPPED:
        raise RuntimeError("cancelled before launch admission")
    before = dict(control.power_snapshot(), phase="before", arm=label)
    observations.append(before)
    control.require_same_power(observations, require_pair=False)
    process = None
    entry = {"qualified": False, "exitCode": None, "terminationConfirmed": False,
             "watchdogExpired": False, "powerObservationError": None,
             "command": command, "startedUtc": before["utc"]}
    try:
        fd = os.open(folder / "run.log", os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        with os.fdopen(fd, "wb") as log:
            if control.STOPPED:
                raise RuntimeError("cancelled before launch admission")
            process = launcher(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                               start_new_session=True, env={**os.environ,
                               "EVERLAST_BENCHMARK_REVISION": revision,
                               "EVERLAST_MLX_SEGMENT_DIAGNOSTICS": "1"})
            control.ACTIVE = process
            entry["wrapperPid"] = process.pid
            write(folder, "started.json", {"command": command, "wrapperPid": process.pid,
                                              "beforePower": before, "sourceRevision": revision})
            if control.STOPPED:
                control.stop(signal.SIGTERM, None)
            deadline = time.monotonic() + timeout_seconds
            next_power = time.monotonic() + sample_interval_seconds
            while process.poll() is None and not control.STOPPED:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    entry["watchdogExpired"] = True
                    break
                try:
                    process.wait(timeout=min(.25, remaining))
                except subprocess.TimeoutExpired:
                    pass
                if time.monotonic() >= next_power:
                    observations.append(dict(control.power_snapshot(), phase="sample", arm=label))
                    control.require_same_power(observations, require_pair=False)
                    next_power = time.monotonic() + sample_interval_seconds
    finally:
        try:
            if process is not None:
                if control.STOPPED and process.poll() is None:
                    try:
                        process.wait(timeout=1)
                    except subprocess.TimeoutExpired:
                        pass
                entry.update(control.reap(process))
                entry["exitCode"] = process.returncode
        finally:
            control.ACTIVE = None
            if entry["terminationConfirmed"]:
                try:
                    observations.append(dict(control.power_snapshot(), phase="after", arm=label))
                except Exception as error:
                    entry["powerObservationError"] = str(error)
            entry["cancelled"] = control.STOPPED
            entry["finishedUtc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            entry["powerObservations"] = [r for r in observations if r["arm"] == label]
            write(folder, "owner-completion.json", entry)
    assert entry["exitCode"] == 0 and entry["processReaped"] and entry["terminationConfirmed"]
    assert not entry["forcedGroupCleanup"] and not entry["cancelled"] and not entry["watchdogExpired"]
    assert entry["powerObservationError"] is None
    control.require_same_power(observations)
    control.require_arm_power(observations, label)
    return entry
