#!/usr/bin/env python3
"""Run only the frozen 94-case remainder serially; no retries or subset statistics."""
import datetime
import json
import os
from pathlib import Path
import tempfile
import math
import stat
import subprocess
import signal
import time

_ACTIVE_PROCESS = None
_INTERRUPTED = False


def write_private(path, value):
    data = (json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n").encode("utf-8")
    write_private_bytes(path, data)


def write_private_bytes(path, data):
    """Atomic publication without replacement, including a concurrent new target."""
    path = Path(path)
    descriptor, staged = tempfile.mkstemp(prefix=".remainder-", dir=path.parent)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.link(staged, path)
    finally:
        os.unlink(staged)


def rows_for(case, status):
    return [{"id": case["id"] + ("::warmup" if index == 0 else "::measured-%03d" % index),
             "runIndex": index, "warmup": index == 0, "status": status}
            for index in range(11)]


def paths_for(output, index, case):
    base = Path(output) / ("%03d-%s.json" % (index + 1, case["id"]))
    return {"results": base, "metadata": Path(str(base) + ".metadata.json"),
            "log": Path(str(base) + ".command.log"),
            "completion": Path(str(base) + ".completion.json")}


def execute(plan, launch):
    """Execute an already verified plan; launch is the owned-process boundary."""
    check(len(plan["cases"]) == 94, "executor requires the complete94-case plan")
    output = plan["output"]
    output.mkdir(mode=0o700)
    output.chmod(0o700)
    report = {"schemaVersion": 1, "kind": "paced-remainder-execution", "cases": [],
              "freezeSha256": plan.get("freezeSha256"), "completed": False,
              "statistics": None, "L": None, "qualified": False}
    if "freezeBytes" in plan:
        write_private_bytes(output / "freeze.json", plan["freezeBytes"])
        report["freezeSnapshot"] = small_binding(output / "freeze.json")
    if "references" in plan:
        write_private(output / "reference-manifest.json", {"schemaVersion": 1,
                      "unicodeVersion": "13.0.0", "cases": plan["references"]})
        report["referenceManifest"] = small_binding(output / "reference-manifest.json")
    failure = None
    for index, case in enumerate(plan["cases"]):
        entry = {"id": case["id"], "caseIndex": index, "status": "unavailable",
                 "error": failure, "rows": rows_for(case, "unavailable")}
        if failure is None and _INTERRUPTED:
            failure = "runner cancellation requested; no further cases launched"
            entry["error"] = failure
        if failure is None:
            paths = paths_for(output, index, case)
            try:
                check_unchanged(plan)
                launched = launch(case, paths)
            except (OSError, ValueError):
                launched = {"exitCode": None, "launchError": "prelaunch binding or process launch failed", "wrapperPid": None}
            validation = validate_case(plan, case, paths) if launched.get("exitCode") == 0 else {"rows": rows_for(case, "missing"), "errors": []}
            # Even a nonzero probe exit may leave partial raw rows; preserve them.
            if launched.get("exitCode") != 0 and paths["results"].exists() and paths["metadata"].exists():
                validation = validate_case(plan, case, paths)
            errors = []
            if launched.get("launchError"):
                errors.append(launched["launchError"])
            if launched.get("exitCode") != 0:
                errors.append("wrapped probe did not exit successfully")
            if launched.get("terminationConfirmed") is not True:
                errors.append("owned process termination not confirmed")
            errors.extend(validation["errors"])
            artifacts = raw_bindings(paths)
            if any(b.get("error") or b.get("sha256") is None for b in artifacts):
                errors.append("raw artifact missing, nonprivate, oversized or changed")
            failure = "; ".join(errors) if errors else None
            entry.update(status="failed" if failure else "ok", error=failure, rows=validation["rows"])
            completion = {"schemaVersion": 1, "caseId": case["id"], "caseIndex": index,
                          "freezeSha256": plan.get("freezeSha256"), "launch": launched,
                          "actualProbePid": validation.get("processId"), "errors": errors,
                          "rows": validation["rows"], "rawFiles": artifacts}
            write_private(paths["completion"], completion)
            entry["completion"] = small_binding(paths["completion"])
        report["cases"].append(entry)
    report["completed"] = failure is None and not _INTERRUPTED
    report["rowCount"] = sum(len(c["rows"]) for c in report["cases"])
    write_private(output / "completion.json", report)
    return report


def check_unchanged(plan):
    # Initial SHA is verified on every source/input. At each launch, fail closed
    # on any inode/size/mode/mtime/ctime change instead of rereading models94times.
    # This detects ordinary mutation, not hostile same-user concurrent writers.
    for path, expected in plan.get("fingerprints", {}).items():
        check(fingerprint(os.stat(path, follow_symlinks=False)) == expected,
              "preflight-bound file changed before a case launch")


def small_binding(path):
    data = bounded_bytes(path, MAX_JSON)
    info = os.stat(path, follow_symlinks=False)
    return {"path": str(path), "bytes": len(data), "mode": oct(stat.S_IMODE(info.st_mode)), "sha256": sha256(data)}


def raw_bindings(paths):
    bindings = []
    for key, maximum in (("results", MAX_TEXT), ("metadata", MAX_METADATA), ("log", MAX_TEXT)):
        path = paths[key]
        try:
            info = os.stat(path, follow_symlinks=False)
            binding = {"path": str(path), "bytes": info.st_size, "mode": oct(stat.S_IMODE(info.st_mode)), "sha256": None}
            if stat.S_ISREG(info.st_mode) and info.st_size <= maximum and stat.S_IMODE(info.st_mode) == 0o600:
                import hashlib
                digest = hashlib.sha256()
                with path.open("rb") as stream:
                    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                        digest.update(chunk)
                binding["sha256"] = digest.hexdigest()
            else:
                binding["error"] = "raw file exceeds bound, is nonregular or not0600"
            bindings.append(binding)
        except OSError:
            bindings.append({"path": str(path), "sha256": None, "error": "raw file unavailable"})
    return bindings


# A failed300s case can still retain11 * 15000 publication records, including
# diagnostics/policy snapshots. 512 MiB permits ~3 KiB per record plus headers;
# 16 MiB is deliberately reserved for the separate text result, not metadata.
MAX_METADATA = 512 * 1024 * 1024
MAX_TEXT = 16 * 1024 * 1024
MAX_JSON = 16 * 1024 * 1024


def bounded_bytes(path, maximum):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > maximum:
            raise ValueError("input is not a bounded regular file")
        data = stream.read(min(maximum + 1, before.st_size + 1))
        if len(data) > maximum or fingerprint(before) != fingerprint(os.fstat(stream.fileno())):
            raise ValueError("input exceeded its bound or changed while read")
        return data


def fingerprint(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns, info.st_mode)


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate JSON field")
        value[key] = item
    return value


def parse_json(data):
    try:
        return json.loads(data, object_pairs_hook=unique_object,
                          parse_constant=lambda _: fail("non-finite JSON number"))
    except (UnicodeError, json.JSONDecodeError):
        raise ValueError("invalid bounded UTF-8 JSON") from None


def read_json(path, maximum):
    return parse_json(bounded_bytes(path, maximum))


def fail(message):
    raise ValueError(message)


def check(condition, message):
    if not condition:
        fail(message)


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def validate_publisher(row):
    check(row["publisherJoined"] is True, "publisher join missing")
    check(row["qosEntered"] is False and row["qosFinished"] is False and row["qosError"] is None, "TC misreported as QoS override")
    policy = row["publisher"]
    check(policy["policy"] == "timeConstrained", "TC policy evidence missing")
    check(all(policy[k] is None for k in ("setupError", "postError", "finishError")), "policy API/cleanup error")
    requested = policy["requested"]
    check(isinstance(requested, dict) and set(requested) == {"periodTicks", "computationTicks", "constraintTicks", "preemptible"}, "requested TC fields missing")
    check(requested["preemptible"] is True and all(integer(requested[k], 1) and requested[k] <= 0xffffffff for k in ("periodTicks", "computationTicks", "constraintTicks")), "invalid requested TC values")
    snapshots = [policy["configured"]]
    for key in ("beforeFeed", "afterFeed"):
        check(policy[key]["error"] is None, "policy snapshot error")
        snapshots.append(policy[key]["snapshot"])
    snapshots.append(policy["after"])
    check(policy["outliers"] == [], "successful feed has unexpected outlier evidence")
    previous_cpu = None
    for snapshot in [policy["before"]] + snapshots:
        check(isinstance(snapshot, dict), "policy snapshot missing")
        constraint = snapshot["timeConstraint"]
        check(type(snapshot["defaultPolicy"]) is bool and type(snapshot["basicPolicy"]) is int, "raw policy fields invalid")
        check(type(constraint["preemptible"]) is bool and all(integer(constraint[k]) and constraint[k] <= 0xffffffff for k in ("periodTicks", "computationTicks", "constraintTicks")), "raw TC fields invalid")
        cpu = snapshot["cpu"]
        check(all(integer(cpu[k]) for k in ("userNanos", "systemNanos")), "CPU observation invalid")
        if previous_cpu is not None:
            check(all(cpu[k] >= previous_cpu[k] for k in ("userNanos", "systemNanos")), "CPU observation moved backwards")
        previous_cpu = cpu
    # Initial `before` precedes policy setup, so it is intentionally exempt.
    for snapshot in snapshots:
        check(snapshot["defaultPolicy"] is False and snapshot["basicPolicy"] == 2 and snapshot["timeConstraint"] == requested, "TC demotion/mismatch observed")


PRETRANSCRIPTION_IDS = {
    "p06-pretranscription-reference-v1": False,
    "p06-pretranscription-candidate-v1": True,
}
_TRACE_VALIDATOR = None


def validate_parameter_mode(parameters):
    allowed = {"paced-full-recording-native-timestamps-rtf-tc-owned-v6"} | set(PRETRANSCRIPTION_IDS)
    check(parameters["configurationId"] in allowed and
          parameters["measurement"]["publisherThread"]["policy"] == "timeConstrained",
          "explicit TC parameter variant required")


def pretranscription_variant(parameters):
    return PRETRANSCRIPTION_IDS.get(parameters.get("configurationId"))


def validate_pretranscription_trace(evidence, row, frames, language):
    """Reuse the screening's pure bounded range/admission/settlement contract.

    Lazy loading avoids its reverse common-helper import at module initialization.
    This source is separately required in pretranscription freeze source bindings.
    No process, audio or model operation runs during import.
    """
    global _TRACE_VALIDATOR
    if _TRACE_VALIDATOR is None:
        import importlib.util
        spec = importlib.util.spec_from_file_location(
            "remainder_screening_trace", Path(__file__).with_name("run-pretranscription-screening.py"))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _TRACE_VALIDATOR = module.validate_trace
    return _TRACE_VALIDATOR(evidence, row, frames, language)


def validate_case(plan, case, paths):
    canonical = rows_for(case, "missing")
    actual_pid = None
    outcome = case["expectedOutcome"]
    try:
        results = read_json(paths["results"], MAX_TEXT)
        metadata = read_json(paths["metadata"], MAX_METADATA)
        for path in (paths["results"], paths["metadata"]):
            check(stat.S_IMODE(os.stat(path, follow_symlinks=False).st_mode) == 0o600, "report mode is not0600")
        check(isinstance(results, list), "result rows are not a list")
        for index, result in enumerate(results[:11]):
            status = result.get("status") if isinstance(result, dict) else "invalid"
            canonical[index]["status"] = status if status in ("ok", "failed", "invalid_setup", "unavailable") else "invalid"
            canonical[index]["validated"] = False
        check(len(results) == 11, "result row count differs from11 (got%d)" % len(results))
        check(all(isinstance(r, dict) for r in results), "malformed result row")
        check(isinstance(metadata, dict), "malformed metadata object")
        check(metadata["schemaVersion"] == 1 and metadata["kind"] == "paced-runtime-complete-recording", "metadata schema/kind mismatch")
        check(metadata["sourceRevision"] == plan["sourceRevision"], "source revision mismatch")
        check(metadata["fixtureSha256"] == case["audio"]["sha256"] and metadata["inputFrames"] == case["audio"]["frames"], "fixture binding mismatch")
        check(metadata["sampleRate"] == 16000 and metadata["channels"] == 1 and metadata["language"] == case["language"], "format/language mismatch")
        check(metadata["model"] == {"id": plan["model"]["id"], "sha256": plan["model"]["sha256"]}, "model binding mismatch")
        check(metadata["parameterDocumentSha256"] == plan["parameterSha256"] and metadata["parameters"] == plan["parameters"], "parameter binding mismatch")
        check(metadata["referenceSha256"] is None, "unexpected exact-text comparison")
        check(metadata["warmupRuns"] == 1 and metadata["measuredRunsRequested"] == 10, "warmup/measured count mismatch")
        check(metadata["debugAssertions"] is False and metadata["qualified"] is False and metadata["L"] is None, "invalid release/qualification flags")
        check(integer(metadata["processId"], 1), "actual processId missing")
        actual_pid = metadata["processId"]
        variant = pretranscription_variant(plan["parameters"])
        if variant is not None:
            check(metadata["pretranscriptionCandidate"] is variant and metadata["contextCleanup"] is None,
                  "mixed pretranscription/context experiment")
        observations = metadata["runs"]
        check(isinstance(observations, list) and len(observations) == 11, "observation row count differs from11")
        for index, (result, row) in enumerate(zip(results, observations)):
            identifier = "warmup" if index == 0 else "measured-%03d" % index
            check(result["id"] == row["id"] == identifier and result["warmup"] is row["warmup"] is (index == 0), "row identity/warmup mismatch")
            check(result["status"] == row["status"] == "ok" and row["error"] is None, "row failure retained")
            check(row["expectedOutcome"] == row["terminal"] == outcome, "unexpected terminal outcome")
            check(integer(row["session"], 1) and row["session"] == index + 1 and row["controllerClockMatchesCapture"] is True, "session/capture clock mismatch")
            check(row["referenceMatch"] is None and row["qosError"] is None, "reference/QoS error")
            validate_publisher(row)
            frames = case["audio"]["frames"]
            elapsed = row["engineElapsedNanos"]
            check(row["engineInputFrames"] == frames and integer(elapsed) and row["engineRtfUnavailableReason"] is None, "actual engine timing missing")
            rtf = row["engineRtf"]
            check(type(rtf) in (int, float) and math.isfinite(rtf) and math.isclose(rtf, elapsed * 16000 / frames / 1e9, rel_tol=1e-12, abs_tol=1e-15), "engine RTF inconsistent")
            feed = row["feed"]
            planned = (frames * 1_000_000_000 + 15999) // 16000
            check(feed["timingValid"] is True and feed["plannedStopOffset"] == {"secs": planned // 1_000_000_000, "nanos": planned % 1_000_000_000}, "feed timing invalid")
            publications = feed["publications"]
            count = (frames + 319) // 320
            check(isinstance(publications, list) and len(publications) == count, "publication count mismatch")
            previous = 0
            for chunk_index, publication in enumerate(publications):
                end = min((chunk_index + 1) * 320, frames)
                deadline = (end * 1_000_000_000 + 15999) // 16000
                check(publication["startFrame"] == chunk_index * 320 and publication["endFrame"] == end and publication["plannedNanos"] == deadline, "publication frame/deadline mismatch")
                before, after = publication["beforePublishNanos"], publication["afterPublishNanos"]
                check(integer(before) and integer(after) and deadline <= before <= after and previous <= before, "publication early/nonmonotone")
                check(0 <= after - deadline <= 10_000_000 and publication["deviationNanos"] == after - deadline and publication["timing"] == "onBudget", "publication timing exceeds budget")
                check("diagnostics" not in publication, "unexpected invalid-publication detail")
                previous = after
            check(feed["waitSummary"]["observedPublications"] == count and feed["waitSummary"]["detailedPublications"] == 0, "wait coverage mismatch")
            stopped, terminal = row["tStopOffsetNanos"], row["tTerminalOffsetNanos"]
            check(integer(stopped) and planned <= stopped <= planned + 10_000_000 and stopped >= previous, "stop timing invalid")
            check(integer(terminal) and terminal >= stopped and row["stopToTerminalNanos"] == terminal - stopped, "terminal timing invalid")
            text = result["text"]
            if outcome == "Empty":
                check(text == "" and row["tTextOffsetNanos"] is None and row["stopToTextNanos"] is None, "Empty has text or L timestamp")
            else:
                check(isinstance(text, str) and bool(text.strip()) and row["tTextOffsetNanos"] == terminal and row["stopToTextNanos"] == terminal - stopped, "Local text/timestamp missing")
            if variant is not None:
                validate_pretranscription_trace(row["jobTrace"], row, frames, case["language"])
                if index:
                    previous_jobs = observations[index-1]["jobTrace"]["trace"]["jobs"]
                    current_jobs = row["jobTrace"]["trace"]["jobs"]
                    check(current_jobs[0]["jobId"] > previous_jobs[-1]["jobId"],
                          "job identity reused across sessions")
            canonical[index].update(validated=True, session=row["session"], expectedOutcome=outcome,
                                    engineElapsedNanos=elapsed, engineRtf=rtf, stopToTerminalNanos=terminal-stopped,
                                    stopToTextNanos=row["stopToTextNanos"], publicationCount=count)
        if variant is not None:
            check(metadata["jobTraceAfterCleanupError"] is None and
                  metadata["jobTraceAfterCleanup"] == observations[-1]["jobTrace"]["trace"],
                  "post-join trace changed or unavailable")
        check(metadata["expectedOutcome"] == outcome and metadata["passed"] is True and metadata["loadError"] is None, "case-level setup/outcome failure")
        check(metadata["cleanup"]["joined"] is True and metadata["cleanup"].get("error") is None, "cleanup join/error invalid")
        check(metadata["engineTimingCoverage"] == {"measuredRuns": 10, "withActualEngineTiming": 10}, "engine timing coverage incomplete")
        return {"errors": [], "rows": canonical, "processId": metadata["processId"]}
    except ValueError as error:
        # These messages come from fixed checks/our bounded JSON parser only.
        return {"errors": [str(error)], "rows": canonical, "processId": actual_pid}
    except (OSError, KeyError, TypeError, AttributeError, IndexError, OverflowError):
        return {"errors": ["case report missing or malformed"], "rows": canonical, "processId": actual_pid}

SELECTION_SHA = "dfb0f38743cd2a67071b327eb0da7251b0655e626edbad90fe04da88b7cd3e2c"
MODEL_SHA = "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2"
VAD_SHA = "29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf"
WATCHDOG_SECONDS = 7200  # Runner liveness only; never an L/quality threshold.
OWN_SOURCES = {"scripts/run-paced-remainder.py", "tests/quality/test_paced_remainder.py",
               "docs/verification/paced-remainder-freeze-schema.md"}


def sha256(data):
    import hashlib
    return hashlib.sha256(data).hexdigest()


def relative_path(root, value):
    check(isinstance(value, str) and 0 < len(value.encode("utf-8")) <= 4096, "invalid relative path")
    path = Path(value)
    check(not path.is_absolute() and all(p not in (".", "..") for p in path.parts), "path must stay inside repository")
    resolved = (root / path).resolve()
    check(root == resolved or root in resolved.parents, "path resolves outside repository")
    return root / path


def verify_binding(root, binding, fingerprints, maximum=None, read=False):
    import hashlib
    path = relative_path(root, binding["path"])
    expected_size = binding["bytes"]
    check(integer(expected_size) and expected_size <= 1024 * 1024 * 1024, "binding size invalid")
    if maximum is not None:
        check(expected_size <= maximum, "binding exceeds format bound")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "rb") as stream:
        before = os.fstat(stream.fileno())
        check(stat.S_ISREG(before.st_mode) and before.st_size == expected_size, "binding size/type mismatch")
        check(binding["mode"] == oct(stat.S_IMODE(before.st_mode)), "binding mode mismatch")
        digest = hashlib.sha256()
        chunks = [] if read else None
        remaining = expected_size
        while remaining:
            chunk = stream.read(min(1024 * 1024, remaining))
            check(bool(chunk), "bound file truncated")
            remaining -= len(chunk)
            digest.update(chunk)
            if read:
                chunks.append(chunk)
        check(not stream.read(1) and fingerprint(before) == fingerprint(os.fstat(stream.fileno())), "bound file changed during verification")
        check(digest.hexdigest() == binding["sha256"], "binding SHA256 mismatch")
    fingerprints[path] = fingerprint(before)
    return b"".join(chunks) if read else None


def compiled_sources(value):
    found = {}
    def walk(node):
        if isinstance(node, dict):
            if "filename" in node and "sha256" in node:
                path, digest = node["filename"], node["sha256"]
                check(path not in found or found[path] == digest, "conflicting compiled source pins")
                found[path] = digest
            for child in node.values():
                walk(child)
        elif isinstance(node, list):
            for child in node:
                walk(child)
    walk(value)
    check(bool(found), "compiled source pins absent")
    return found


def preflight(freeze_path, root):
    import io
    import re
    import subprocess
    import wave
    root = Path(root).resolve(strict=True)
    freeze_bytes = bounded_bytes(freeze_path, MAX_JSON)
    freeze = parse_json(freeze_bytes)
    check(freeze["schemaVersion"] == 1 and freeze["kind"] == "prospective-paced-remainder-v1", "freeze schema/kind mismatch")
    revision = freeze["sourceRevision"]
    check(isinstance(revision, str) and re.fullmatch(r"[0-9a-f]{40}", revision), "source revision must be a full Git commit")
    check(freeze["binary"]["path"] == "target/release/examples/paced_runtime_probe", "only paced_runtime_probe is supported")
    check(freeze["selection"]["sha256"] == SELECTION_SHA, "the unchanged94-case selection is required")
    check(freeze["publisherPolicy"] == "timeConstrained", "this freeze requires the explicit TC developer variant")
    watchdog = freeze.get("caseTimeoutSeconds", WATCHDOG_SECONDS)
    check(type(watchdog) is int and watchdog == WATCHDOG_SECONDS, "unsupported runner watchdog")
    fingerprints = {}
    verify_binding(root, freeze["binary"], fingerprints)
    parameter_bytes = verify_binding(root, freeze["parameters"], fingerprints, 1024 * 1024, True)
    parameters = parse_json(parameter_bytes)
    check(parameters["build"]["debugAssertions"] is False, "release parameter configuration required")
    full = parameters["runtime"]["fullParams"]
    check(full["no_timestamps"] is False and full["suppress_nst"] is False, "production decoding configuration mismatch")
    validate_parameter_mode(parameters)
    for key, value in {"chunkMillis": 20, "jitterToleranceNanos": 10_000_000,
                       "maximumSleepNanos": 1_000_000, "finalSpinNanos": 250_000}.items():
        check(parameters["measurement"][key] == value, "fixed clock configuration mismatch")
    pins = compiled_sources(parameters)
    sources = {b["path"]: b for b in freeze["sources"]}
    check(len(sources) == len(freeze["sources"]) and (set(pins) | OWN_SOURCES).issubset(sources), "source list must uniquely include compiled pins plus executor/test/schema")
    if pretranscription_variant(parameters) is not None:
        check("scripts/run-pretranscription-screening.py" in sources,
              "shared pretranscription trace validator source binding missing")
    for path, binding in sources.items():
        data = verify_binding(root, binding, fingerprints, MAX_JSON, True)
        check(path not in pins or pins[path] == binding["sha256"], "compiled/source binding mismatch")
        object_name = revision + ":" + path
        size = subprocess.run(["git", "cat-file", "-s", object_name], cwd=root,
                              stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=10, check=False)
        check(size.returncode == 0 and len(size.stdout) < 32 and size.stdout.strip() == str(len(data)).encode(),
              "immutable source size differs")
        committed = subprocess.run(["git", "show", object_name], cwd=root,
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=10, check=False)
        check(committed.returncode == 0 and sha256(committed.stdout) == sha256(data), "source differs from immutable revision")
    model, vad = freeze["model"], freeze["vad"]
    check(model["id"] == "large-v3-turbo-q5_0" and model["sha256"] == MODEL_SHA and model["bytes"] == 574041195, "reference model identity mismatch")
    check(vad["sha256"] == VAD_SHA and vad["bytes"] == 885098, "VAD identity mismatch")
    check(Path(model["path"]).name == "ggml-large-v3-turbo-q5_0.bin" and Path(vad["path"]).name == "ggml-silero-v5.1.2.bin" and Path(model["path"]).parent == Path(vad["path"]).parent, "model/VAD sibling paths required")
    check(any(m.get("id") == model["id"] and m.get("sha256") == model["sha256"] for m in parameters["models"]), "parameter model binding mismatch")
    check(parameters["runtime"]["speechGate"]["model"]["sha256"] == vad["sha256"], "parameter VAD binding mismatch")
    verify_binding(root, model, fingerprints)
    verify_binding(root, vad, fingerprints)
    selection_bytes = verify_binding(root, freeze["selection"], fingerprints, MAX_JSON, True)
    selection = parse_json(selection_bytes)
    cases = selection["cases"]
    check(len(cases) == 94 and len({c["id"] for c in cases}) == 94, "selection must preserve all94 unique cases")
    manifests = {}
    for binding in selection["sourceManifests"] + [selection["centralManifest"]]:
        # Historical selection bindings specify bytes/SHA, not filesystem modes.
        binding = dict(binding, mode=oct(stat.S_IMODE(relative_path(root, binding["path"]).stat().st_mode)))
        data = verify_binding(root, binding, fingerprints, MAX_JSON, True)
        manifests[binding["path"]] = parse_json(data)
    inputs = {b["path"]: b for b in freeze["inputs"]}
    check(len(inputs) == len(freeze["inputs"]) == 94 and set(inputs) == {c["audio"]["path"] for c in cases}, "all94 input bindings required")
    references = []
    for case in cases:
        check(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", case["id"]), "unsafe case identifier")
        check(case["warmupRuns"] == 1 and case["measuredRuns"] == 10, "case repetition scope changed")
        source = [c for c in manifests[case["sourceManifest"]]["cases"] if c["id"] == case["id"]]
        check(len(source) == 1, "missing/duplicate independent reference")
        source = source[0]
        check(all(source[k] == case[k] for k in ("language", "class", "split", "audio")), "source case fields changed")
        reference = source["reference"]
        check(sha256(reference.encode("utf-8")) == case["referenceUtf8Sha256"], "independent reference SHA mismatch")
        check(case["expectedOutcome"] == ("Local" if reference.strip() else "Empty"), "expected outcome differs from frozen reference")
        audio = case["audio"]
        check(audio["sampleRate"] == 16000 and integer(audio["frames"], 1) and audio["frames"] <= 4_800_000, "unsupported input duration/format")
        binding = inputs[audio["path"]]
        check(binding["sha256"] == audio["sha256"], "selection/input SHA mismatch")
        data = verify_binding(root, binding, fingerprints, 10 * 1024 * 1024, True)
        try:
            with wave.open(io.BytesIO(data), "rb") as wav:
                check((wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes(), wav.getcomptype()) == (1, 2, 16000, audio["frames"], "NONE"), "input WAV header mismatch")
                check(len(wav.readframes(audio["frames"] + 1)) == audio["frames"] * 2, "input WAV payload mismatch")
        except (wave.Error, EOFError):
            fail("invalid bounded PCM WAV")
        references.append({k: source[k] for k in ("id", "language", "class", "split", "reference")})
    check(sum(c["expectedOutcome"] == "Empty" for c in cases) == 6, "six Empty/eighty-eight Local roles required")
    check(Path(freeze["outputDirectory"]).parts[0] == "artifacts", "output must be under ignored artifacts")
    output = relative_path(root, freeze["outputDirectory"])
    check(not output.exists() and not output.is_symlink() and output.parent.is_dir(), "output directory must be new with existing parent")
    return {"root": root, "output": output, "cases": cases, "parameters": parameters,
            "parameterSha256": sha256(parameter_bytes), "sourceRevision": revision, "model": model,
            "binary": relative_path(root, freeze["binary"]["path"]),
            "parameterPath": relative_path(root, freeze["parameters"]["path"]),
            "fingerprints": fingerprints, "references": references, "freezeSha256": sha256(freeze_bytes), "freezeBytes": freeze_bytes}


def command_for(plan, case, paths):
    command = ["/usr/bin/time", "-l", str(plan["binary"]), str(plan["root"] / plan["model"]["path"]),
               "reference", str(plan["root"] / case["audio"]["path"]), case["language"],
               str(plan["parameterPath"]), str(paths["results"]), "10"]
    if case["expectedOutcome"] == "Empty":
        command.append("--expect-empty")
    return command


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def stop_owned_group(process):
    # start_new_session=True made only this time-wrapper and its descendants
    # members of PGID=wrapper.pid. Never signal an inherited/foreign group.
    kill_failed = False
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except OSError:
        kill_failed = True
    try:
        code = process.wait(timeout=10)
    except (subprocess.TimeoutExpired, OSError):
        return None, False
    if kill_failed:
        return code, False
    deadline = time.monotonic() + 2
    while True:
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            return code, True
        except OSError:
            return code, False
        if time.monotonic() >= deadline:
            return code, False
        time.sleep(.05)


def launch_case(plan, case, paths):
    global _ACTIVE_PROCESS
    command = command_for(plan, case, paths)
    result = {"command": command, "startedUtc": utc_now(), "finishedUtc": None,
              "wrapperPid": None, "exitCode": None, "launchError": None,
              "watchdogExpired": False, "terminationConfirmed": False}
    environment = dict(os.environ, EVERLAST_BENCHMARK_REVISION=plan["sourceRevision"])
    process = None
    try:
        descriptor = os.open(paths["log"], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb") as log:
            process = subprocess.Popen(command, cwd=plan["root"], env=environment,
                                       stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            result["wrapperPid"] = process.pid
            _ACTIVE_PROCESS = process
            print(json.dumps({"caseId": case["id"], "wrapperPid": process.pid,
                              "startedUtc": result["startedUtc"]}, separators=(",", ":")), flush=True)
            try:
                if _INTERRUPTED:
                    raise KeyboardInterrupt
                result["exitCode"] = process.wait(timeout=WATCHDOG_SECONDS)
                result["terminationConfirmed"] = True
            except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
                _ACTIVE_PROCESS = None
                result["watchdogExpired"] = isinstance(error, subprocess.TimeoutExpired)
                result["launchError"] = ("runner watchdog expired; not an L/quality failure" if result["watchdogExpired"] else "runner interrupted")
                result["exitCode"], result["terminationConfirmed"] = stop_owned_group(process)
    except OSError:
        _ACTIVE_PROCESS = None
        result["launchError"] = "owned wrapper could not launch or be observed"
        if process is not None:
            result["exitCode"], result["terminationConfirmed"] = stop_owned_group(process)
        else:
            result["terminationConfirmed"] = True  # No child was created.
    _ACTIVE_PROCESS = None
    result["finishedUtc"] = utc_now()
    return result


def request_stop(_signal, _frame):
    global _INTERRUPTED
    _INTERRUPTED = True
    if _ACTIVE_PROCESS is not None:
        raise KeyboardInterrupt


def main(argv=None):
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze", type=Path)
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parents[1]
    try:
        plan = preflight(args.freeze, root)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.SubprocessError):
        print("paced remainder preflight failed; no probe launched")
        return 2
    handlers = {sig: signal.signal(sig, request_stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        try:
            report = execute(plan, lambda case, paths: launch_case(plan, case, paths))
        except (OSError, ValueError):
            print("paced remainder publication failed; existing raw evidence retained; no retry")
            return 1
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
    print("paced remainder complete=%s; all94 slots retained; no transcript printed" % report["completed"])
    return 0 if report["completed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
