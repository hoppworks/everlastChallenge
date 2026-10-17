#!/usr/bin/env python3
"""Record host/provenance before the fixed local resource workload; keep all failures."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def observation(command):
    try:
        result = subprocess.run(command, capture_output=True, timeout=5, check=True)
        return {"value": result.stdout.decode("utf-8").strip(), "error": None}
    except (OSError, subprocess.SubprocessError, UnicodeError) as error:
        return {"value": None, "error": str(error)}


def write_private(path, value):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("model", type=Path)
    parser.add_argument("selection", choices=("reference", "small"))
    parser.add_argument("wav", type=Path)
    parser.add_argument("reference", type=Path)
    parser.add_argument("parameters", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    host_path = Path(str(output) + ".host.json")
    completion_path = Path(str(output) + ".completion.json")
    log_path = Path(str(output) + ".command.log")
    if any(path.exists() for path in (output, host_path, completion_path, log_path)):
        parser.error("all output paths must be new; existing results are never replaced")
    binary, model, wav, reference, parameters = (
        path.resolve(strict=True)
        for path in (args.binary, args.model, args.wav, args.reference, args.parameters)
    )
    command = ["/usr/bin/time", "-l", str(binary), str(model), args.selection,
               str(wav), str(reference), str(parameters), str(output)]
    power = observation(["/usr/bin/pmset", "-g", "batt"])
    if power["value"] is not None:
        power["value"] = ("AC Power" if "AC Power" in power["value"] else
                          "Battery Power" if "Battery Power" in power["value"] else "unknown")
    write_private(host_path, {
        "schemaVersion": 1, "recordedBeforeChildLaunchUtc": utc_now(),
        "command": command,
        "sourceRevision": os.environ.get("EVERLAST_BENCHMARK_REVISION"),
        "launcherSha256": digest(Path(__file__)), "binarySha256": digest(binary),
        "parameterDocumentSha256": digest(parameters), "fixtureSha256": digest(wav),
        "referenceSha256": digest(reference),
        "cpu": observation(["/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"]),
        "memoryBytes": observation(["/usr/sbin/sysctl", "-n", "hw.memsize"]),
        "macOS": observation(["/usr/bin/sw_vers", "-productVersion"]),
        "macOSBuild": observation(["/usr/bin/sw_vers", "-buildVersion"]),
        "rustc": observation(["rustc", "--version"]), "powerSource": power,
        "limits": ["power source is not a measured energy profile",
                   "time -l is an OS process resource observation, not live heap evidence",
                   "no hard preemption of Engine initialization or native inference",
                   "host-command observations have five-second subprocess timeouts"]
    })
    descriptor = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    code = None
    error = None
    with os.fdopen(descriptor, "wb") as log:
        try:
            code = subprocess.run(command, stdout=log, stderr=log, check=False).returncode
        except OSError as failure:
            error = str(failure)
    report_error = None
    try:
        if output.stat().st_size > 8 * 1024 * 1024:
            raise ValueError("workload report exceeds 8 MiB")
        report = json.loads(output.read_text(encoding="utf-8"))
        if not isinstance(report, dict) or report.get("kind") != "runtime-lifecycle-resource-workload":
            raise ValueError("unexpected workload report kind")
        if report.get("behaviorPassed") is not True or report.get("resourceQualified") is not False:
            raise ValueError("workload report does not confirm behavior with qualification still open")
    except (OSError, ValueError, UnicodeError) as failure:
        report_error = str(failure)
    effective_code = code if code not in (None, 0) else (1 if error or report_error else 0)
    write_private(completion_path, {
        "schemaVersion": 1, "completedUtc": utc_now(), "exitCode": code, "launchError": error,
        "effectiveExitCode": effective_code, "reportError": report_error,
        "hostReportSha256": digest(host_path), "commandLogSha256": digest(log_path),
        "workloadReportSha256": digest(output) if output.is_file() else None,
        "resourceQualified": False
    })
    print(f"runtime resource command exit={code}; effectiveExit={effective_code}; report={output}")
    return effective_code


if __name__ == "__main__":
    raise SystemExit(main())
