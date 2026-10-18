#!/usr/bin/env python3
"""Frozen, serial pretranscription pipeline/quality screen; never qualifies L."""
import importlib.util
import math
import json
import os
import stat
import datetime
import subprocess
import signal
import time
import re
import sys
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location("paced_remainder", Path(__file__).with_name("run-paced-remainder.py"))
common = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(common)


check = common.check
integer = common.integer
MAX_JOBS = 601


def validate_retired(job, current_session):
    if job is None:
        return
    check(integer(job["session"], 1) and job["session"] < current_session, "retired identity is not old")
    check(integer(job["jobId"], 1) and job["generation"] == 1, "invalid retired job identity")
    queued, settled = job["serviceAcceptedOffsetNanos"], job["workerSettledOffsetNanos"]
    check(integer(queued) and integer(settled) and queued <= settled, "retired job lacks post-join settlement")
    begin, end = job["workerBeforeDecodeOffsetNanos"], job["workerAfterDecodeOffsetNanos"]
    if begin is None:
        check(end is None and job["workerOutcome"] in ("cancelled-pending-no-decode", "cancelled-before-decode", "discarded-closing"),
              "unstarted retired job claims decoder work")
    else:
        check(integer(begin) and integer(end) and queued <= begin <= end <= settled,
              "retired worker chronology missing")
    check(job["adoption"] != "accepted", "old retired result was accepted")


def validate_trace(evidence, row, frames, language):
    trace = evidence["trace"]
    session = row["session"]
    check(integer(session, 1), "invalid current session")
    check(evidence["error"] is None and trace["error"] is None, "job trace reports failure")
    check(trace["enabled"] is True and trace["session"] == session and trace["maximumJobs"] == MAX_JOBS,
          "trace session/capacity differs")
    check(trace["overflowEvents"] == 0 and trace["firstOverflowJob"] is None, "job trace overflow")
    jobs, proofs = trace["jobs"], evidence["inputProofs"]
    check(isinstance(jobs, list) and 1 <= len(jobs) <= MAX_JOBS and len(proofs) == len(jobs), "incomplete bounded job trace")
    publications = row["feed"]["publications"]
    covered = previous_id = total_span = 0
    for job, proof in zip(jobs, proofs):
        check(type(job["session"]) is int and type(job["generation"]) is int and job["session"] == session and job["generation"] == 1 and job["language"] == language,
              "job session/generation/language differs")
        jid, start, end = job["jobId"], job["timelineStart"], job["timelineEnd"]
        check(integer(jid, 1) and jid > previous_id and integer(start) and integer(end) and
              start == covered and start < end <= frames, "job timeline gap/overlap or identity mismatch")
        local_start, local_end = job["storageStart"], job["storageEnd"]
        check(integer(local_start) and integer(local_end) and local_start < local_end <= 4_800_000 and
              end-start == local_end-local_start == job["inputFrames"], "local storage and timeline lengths differ")
        times = [job[k] for k in ("hostSubmitOffsetNanos", "serviceAcceptedOffsetNanos",
                 "workerBeforeDecodeOffsetNanos", "workerAfterDecodeOffsetNanos", "workerSettledOffsetNanos",
                 "resultAdoptedOffsetNanos")]
        check(all(integer(t) for t in times) and times == sorted(times), "missing/nonmonotone job edges")
        check(times[-1] <= row["tTerminalOffsetNanos"] and job["workerOutcome"] == "decoded-ok" and
              job["adoption"] == "accepted", "job did not finish and adopt before terminal")
        check(type(job["audioReleasedAtSubmit"]) is bool, "missing audio-release role")
        required = end if job["audioReleasedAtSubmit"] else end + 4800
        check(required <= frames and job["requiredStableFrames"] == required, "wrong stable-frame boundary")
        through, last = 0, None
        for publication in publications:
            if publication["afterPublishNanos"] > times[0]:
                break
            check(publication["startFrame"] == through, "publication gap in input proof")
            through = publication["endFrame"]
            last = publication["afterPublishNanos"]
        check(through >= required, "job admitted before required audio was completely published")
        check(proof == {"jobId": jid, "requiredStableFrames": required,
                        "publishedThroughFramesAtAdmission": through,
                        "lastCompletePublicationOffsetNanos": last}, "reported input proof differs from independent reconstruction")
        total_span += times[3] - times[2]
        covered, previous_id = end, jid
    check(jobs[-1]["audioReleasedAtSubmit"] is True, "no final audio-release job")
    check(covered == frames and type(row["engineInputFrames"]) is int and row["engineInputFrames"] == covered, "aggregate decoder frames do not cover input")
    elapsed = row["engineElapsedNanos"]
    check(integer(elapsed) and elapsed <= total_span, "actual decoder duration exceeds enclosing worker spans")
    check(type(row["engineRtf"]) in (int, float) and math.isfinite(row["engineRtf"]) and
          math.isclose(row["engineRtf"], elapsed * 16000 / frames / 1e9, rel_tol=1e-12, abs_tol=1e-15), "engine RTF differs")
    validate_retired(trace["retiredJob"], session)
    return {"jobCount": len(jobs), "inputFrames": covered, "workerSpanNanos": total_span,
            "engineElapsedNanos": elapsed}


def canonical_rows(case, status):
    return [{"id": case["id"], "runId": run, "warmup": run == "warmup", "status": status}
            for run in ("warmup", "measured-001")]


def paths_for(plan, case):
    return {key: common.relative_path(plan["root"], case[key])
            for key in ("output", "metadata", "log", "completion")}


def execute(plan, launch, finish=None):
    check(len(plan["blocks"]) == 4 and all(len(b["cases"]) == 100 for b in plan["blocks"]), "expected400 frozen commands")
    output = plan["output"]
    output.mkdir(mode=0o700)
    output.chmod(0o700)
    if "freezeBytes" in plan:
        common.write_private_bytes(output / "freeze.json", plan["freezeBytes"])
    if "launchObservation" in plan:
        common.write_private(output / "launch.json", plan["launchObservation"])
    report = {"schemaVersion": 1, "kind": "pretranscription-screening-execution", "blocks": [],
              "freezeSha256": plan.get("freezeSha256"), "completed": False,
              "qualified": False, "L": None, "error": None}
    failure = None
    for block in plan["blocks"]:
        block_report = {"name": block["name"], "variant": block["variant"], "cases": []}
        for case in block["cases"]:
            entry = {"id": case["id"], "status": "unavailable", "error": failure,
                     "rows": canonical_rows(case, "unavailable")}
            if failure is None and _INTERRUPTED:
                failure = "runner interrupted; no subsequent cases launched"
                entry["error"] = failure
            if failure is None:
                paths = paths_for(plan, case)
                try:
                    common.check_unchanged(plan)
                    launched = launch(case, paths)
                except (OSError, ValueError, KeyboardInterrupt) as error:
                    launched = {"exitCode": None, "terminationConfirmed": False,
                                "launchError": "prelaunch/launch failed: " + type(error).__name__}
                errors = []
                if launched.get("exitCode") != 0:
                    errors.append("wrapped probe did not exit successfully")
                if launched.get("terminationConfirmed") is not True:
                    errors.append("owned process termination unconfirmed")
                if launched.get("launchError"):
                    errors.append(launched["launchError"])
                validation = validate_case(plan, block, case, paths)
                errors.extend(validation["errors"])
                raw = common.raw_bindings({"results": paths["output"], "metadata": paths["metadata"], "log": paths["log"]})
                if any(item.get("error") or item.get("sha256") is None for item in raw):
                    errors.append("raw artifacts missing, nonprivate, changed or oversized")
                failure = "; ".join(errors) if errors else None
                entry.update(status="failed" if failure else "ok", error=failure,
                             rows=validation["rows"], launch=launched, rawFiles=raw,
                             actualProbePid=validation.get("processId"))
                if validation.get("processId") is not None:
                    if validation["processId"] in plan.setdefault("seenPids", set()):
                        failure = "actual probe PID reused within frozen sequence"
                        entry.update(status="failed", error=failure)
                    plan["seenPids"].add(validation["processId"])
            common.write_private(paths_for(plan, case)["completion"], entry)
            block_report["cases"].append(entry)
        report["blocks"].append(block_report)
    if finish is not None:
        power_ended = finish()
        report["powerAssertionEnded"] = power_ended
        if not power_ended:
            failure = failure or "owned idle-sleep assertion termination unconfirmed"
    report.update(completed=failure is None, error=failure, rowCount=800)
    common.write_private(output / "completion.json", report)
    return report


def validate_case(plan, block, case, paths):
    rows = canonical_rows(case, "missing")
    actual_pid = None
    try:
        results = common.read_json(paths["output"], common.MAX_TEXT)
        meta = common.read_json(paths["metadata"], common.MAX_METADATA)
        for index, item in enumerate(results[:2]):
            if isinstance(item, dict):
                rows[index]["status"] = item.get("status", "invalid")
        check(len(results) == 2 and len(meta["runs"]) == 2, "expected warmup plus one measured row")
        actual_pid = meta["processId"]
        check(integer(actual_pid, 1), "missing actual probe PID")
        check(meta["sourceRevision"] == plan["sourceRevision"] and meta["schemaVersion"] == 1 and
              meta["kind"] == "paced-runtime-complete-recording", "source/schema differs")
        check(meta["fixtureSha256"] == case["audio"]["sha256"] and meta["inputFrames"] == case["audio"]["frames"] and
              (meta["sampleRate"], meta["channels"], meta["language"]) == (16000, 1, case["language"]), "input identity differs")
        check(meta["model"] == {"id": plan["model"]["id"], "sha256": plan["model"]["sha256"]}, "model identity differs")
        variant = block["variant"]
        check(meta["parameters"] == plan["parameters"][variant] and
              meta["parameterDocumentSha256"] == plan["parameterBindings"][variant]["sha256"], "parameters differ")
        check(meta["pretranscriptionCandidate"] is (variant == "PretransCandidate") and
              meta["contextCleanup"] is None and meta["referenceSha256"] is None, "mixed experiment/reference input")
        check(meta["warmupRuns"] == meta["measuredRunsRequested"] == 1 and meta["debugAssertions"] is False and
              meta["qualified"] is False and meta["L"] is None, "counts or qualification differs")
        check(meta["passed"] is True and meta["loadError"] is None and
              meta["cleanup"]["joined"] is True and meta["cleanup"]["error"] is None, "setup or runtime cleanup failed")
        check(meta["expectedOutcome"] == case["expectedOutcome"] and
              meta["engineTimingCoverage"] == {"measuredRuns": 1, "withActualEngineTiming": 1}, "outcome/timing coverage differs")
        summaries = []
        for index, (result, row) in enumerate(zip(results, meta["runs"])):
            run = "warmup" if index == 0 else "measured-001"
            check(set(result) == {"id", "warmup", "status", "text"} and
                  result["id"] == row["id"] == run and result["warmup"] is row["warmup"] is (index == 0), "row identity differs")
            check(result["status"] == row["status"] == "ok" and row["error"] is None and
                  row["session"] == index+1 and row["controllerClockMatchesCapture"] is True, "row failed/session differs")
            check(row["expectedOutcome"] == row["terminal"] == case["expectedOutcome"] and row["referenceMatch"] is None, "terminal/reference differs")
            common.validate_publisher(row)
            validate_feed(row, case["audio"]["frames"])
            terminal, stopped = row["tTerminalOffsetNanos"], row["tStopOffsetNanos"]
            check(integer(terminal) and terminal >= stopped and row["stopToTerminalNanos"] == terminal-stopped, "terminal clock differs")
            if case["expectedOutcome"] == "Empty":
                check(result["text"] == "" and row["tTextOffsetNanos"] is None and row["stopToTextNanos"] is None, "Empty claimed text/L")
            else:
                check(isinstance(result["text"], str) and bool(result["text"].strip()) and
                      row["tTextOffsetNanos"] == terminal and row["stopToTextNanos"] == terminal-stopped, "Local lacks final text/clock")
            check(row["engineRtfUnavailableReason"] is None, "engine timing unavailable")
            if index:
                previous_jobs = meta["runs"][index-1]["jobTrace"]["trace"]["jobs"]
                current_jobs = row["jobTrace"]["trace"]["jobs"]
                check(bool(previous_jobs) and bool(current_jobs) and current_jobs[0]["jobId"] > previous_jobs[-1]["jobId"],
                      "job IDs reset or overlap across sessions")
            summaries.append(validate_trace(row["jobTrace"], row, case["audio"]["frames"], case["language"]))
            rows[index].update(validated=True, session=row["session"], **summaries[-1])
        late = meta["jobTraceAfterCleanup"]
        check(meta["jobTraceAfterCleanupError"] is None and late == meta["runs"][-1]["jobTrace"]["trace"],
              "post-join trace missing, changed or errored")
        return {"errors": [], "rows": rows, "processId": actual_pid}
    except ValueError as error:
        return {"errors": [str(error)], "rows": rows, "processId": actual_pid}
    except (OSError, KeyError, TypeError, AttributeError, IndexError, OverflowError):
        return {"errors": ["missing/malformed bounded report"], "rows": rows, "processId": actual_pid}


def validate_feed(row, frames):
    feed = row["feed"]
    planned = (frames * 1_000_000_000 + 15999) // 16000
    check(feed["timingValid"] is True and feed["plannedStopOffset"] == {"secs": planned//1_000_000_000, "nanos": planned%1_000_000_000}, "feed timing invalid")
    pubs = feed["publications"]
    count = (frames+319)//320
    check(len(pubs) == count and feed["waitSummary"]["observedPublications"] == count and
          feed["waitSummary"]["detailedPublications"] == 0, "incomplete publication coverage")
    previous = 0
    for i, publication in enumerate(pubs):
        end = min((i+1)*320, frames)
        deadline = (end*1_000_000_000+15999)//16000
        before, after = publication["beforePublishNanos"], publication["afterPublishNanos"]
        check(publication["startFrame"] == i*320 and publication["endFrame"] == end and
              publication["plannedNanos"] == deadline, "publication frame/deadline differs")
        check(integer(before) and integer(after) and previous <= before and deadline <= before <= after <= deadline+10_000_000 and
              publication["deviationNanos"] == after-deadline and publication["timing"] == "onBudget" and
              "diagnostics" not in publication, "invalid publication timing")
        previous = after
    stopped = row["tStopOffsetNanos"]
    check(integer(stopped) and previous <= stopped and planned <= stopped <= planned+10_000_000, "Stop clock differs")


VARIANTS = ("PretransReference", "PretransCandidate", "PretransCandidate", "PretransReference")
SELECTION_SHA = "864332ed3e11c4924066f920ff8c98e2c2c98f6040ddf0bff53ceff3b40551a9"
MANIFEST_SHA = "df432b44c48a39ebe290c6bd20d9ba8b800fd2211cdd898c86a9b22a54edf030"
MODEL_PATH = "artifacts/models/ggml-large-v3-turbo-q5_0.bin"
VAD_PATH = "artifacts/models/ggml-silero-v5.1.2.bin"
WATCHDOG_SECONDS = 7200  # liveness only, never an L threshold
OWN_SOURCES = {"scripts/run-pretranscription-screening.py", "tests/quality/test_pretranscription_screening.py",
               "scripts/run-paced-remainder.py"}


def validate_blocks(freeze, root, cases):
    blocks = freeze["blocks"]
    check(len(blocks) == 4 and tuple(b["variant"] for b in blocks) == VARIANTS, "ABBA order differs")
    check(len({b["name"] for b in blocks}) == 4, "duplicate block name")
    seen_paths = set()
    output = common.relative_path(root, freeze["outputDirectory"])
    for block in blocks:
        check(len(block["cases"]) == 100, "block does not contain100 cases")
        parameter = common.relative_path(root, freeze["parameters"][block["variant"]]["path"])
        for entry, case in zip(block["cases"], cases):
            check(set(entry) == {"id", "command", "output", "metadata", "log", "completion"}, "unexpected command-entry fields")
            check(entry["id"] == case["id"], "case order differs")
            paths = {k: common.relative_path(root, entry[k]) for k in ("output", "metadata", "log", "completion")}
            for path in paths.values():
                check(path.parent == output and path not in seen_paths and not os.path.lexists(path), "output path occupied, duplicated or outside new directory")
                seen_paths.add(path)
            check(paths["metadata"] == Path(str(paths["output"])+".metadata.json"), "automatic metadata path differs")
            command = ["/usr/bin/time", "-l", str(common.relative_path(root, freeze["binary"]["path"])),
                       str(root/MODEL_PATH), "reference", str(common.relative_path(root, case["audio"]["path"])),
                       case["language"], str(parameter), str(paths["output"]), "1"]
            if case["expectedOutcome"] == "Empty":
                command.append("--expect-empty")
            check(entry["command"] == command, "command differs from frozen one-warmup/one-measured protocol")
    return blocks


def immutable_source(root, revision, path, data):
    # Size check bounds git-show output before invoking it.
    obj = revision+":"+path
    size = subprocess.run(["git", "cat-file", "-s", obj], cwd=root, capture_output=True, timeout=10, check=False)
    check(size.returncode == 0 and size.stdout.strip() == str(len(data)).encode(), "immutable source size differs")
    result = subprocess.run(["git", "show", obj], cwd=root, capture_output=True, timeout=10, check=False)
    check(result.returncode == 0 and common.sha256(result.stdout) == common.sha256(data), "source differs from immutable revision")


def preflight(freeze_path, root):
    import io
    import wave
    root = Path(root).resolve(strict=True)
    raw = common.bounded_bytes(freeze_path, common.MAX_JSON)
    freeze = common.parse_json(raw)
    check(freeze["schemaVersion"] == 1 and freeze["kind"] == "prospective-p06-pretranscription-screening-v1", "wrong freeze schema")
    revision = freeze["sourceRevision"]
    check(isinstance(revision, str) and re.fullmatch("[0-9a-f]{40}", revision), "full source revision required")
    check(freeze["caseTimeoutSeconds"] == WATCHDOG_SECONDS, "watchdog scope differs")
    check(freeze["binary"]["path"] == "target/release/examples/paced_runtime_probe", "wrong probe binary")
    check(freeze["manifest"]["sha256"] == MANIFEST_SHA and freeze["selection"]["sha256"] == SELECTION_SHA, "fixed100 input selection changed")
    fingerprints = {}
    freeze_path = Path(freeze_path).resolve(strict=True)
    check(root in freeze_path.parents, "freeze must be inside repository")
    freeze_binding = dict(path=str(freeze_path.relative_to(root)), bytes=len(raw), sha256=common.sha256(raw),
                          mode=oct(stat.S_IMODE(freeze_path.stat().st_mode)))
    common.verify_binding(root, freeze_binding, fingerprints, common.MAX_JSON)
    def verify(binding, maximum=common.MAX_JSON, read=True):
        return common.verify_binding(root, binding, fingerprints, maximum, read)
    verify(freeze["binary"], 128*1024*1024, False)
    check(os.access(root/freeze["binary"]["path"], os.X_OK), "probe is not executable")
    manifest = common.parse_json(verify(freeze["manifest"]))
    selection = common.parse_json(verify(freeze["selection"]))
    check(manifest["unicodeVersion"] == "13.0.0", "reference Unicode contract differs")
    cases = selection["cases"]
    check(len(cases) == len(manifest["cases"]) == 100 and len({c["id"] for c in cases}) == 100, "100 unique cases required")
    check(sum(c["expectedOutcome"] == "Empty" for c in cases) == 6, "six frozen Empty cases required")
    check(set(freeze["parameters"]) == set(VARIANTS), "exactly two parameter arms required")
    parameters, pins = {}, {}
    for variant, binding in freeze["parameters"].items():
        document = common.parse_json(verify(binding, 1024*1024))
        candidate = variant == "PretransCandidate"
        check(document["configurationId"] == "p06-pretranscription-"+("candidate" if candidate else "reference")+"-v1" and
              document["pretranscription"]["candidate"] is candidate, "wrong explicitly selected parameter arm")
        check(document["build"]["debugAssertions"] is False and document["runtime"]["context"]["flash_attn"] is False and
              document["runtime"]["fullParams"]["audio_ctx"] == 0 and document["runtime"]["fullParams"]["no_timestamps"] is False and
              document["runtime"]["fullParams"]["suppress_nst"] is False, "decoder/context policy differs")
        for k,v in {"windowFrames":320,"startRms":0.00005,"endRms":0.000025,"prerollFrames":2400,
                    "tailFrames":3200,"confirmationFrames":8000,"maximumJobs":601}.items():
            check(document["pretranscription"][k] == v, "fixed pretranscription policy differs")
        for k,v in {"chunkMillis":20,"jitterToleranceNanos":10_000_000,"maximumSleepNanos":1_000_000,"finalSpinNanos":250_000}.items():
            check(document["measurement"][k] == v, "clock policy differs")
        check(document["measurement"]["publisherThread"]["policy"] == "timeConstrained", "explicit TC required")
        flag = "--print-pretrans-candidate-parameters" if candidate else "--print-pretrans-reference-parameters"
        printed = subprocess.run([str(root/freeze["binary"]["path"]), flag], cwd=root,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=False)
        check(printed.returncode == 0 and len(printed.stdout) <= 1024*1024 and
              common.parse_json(printed.stdout) == document, "parameters differ from compiled template")
        parameters[variant] = document
        for path, digest in common.compiled_sources(document).items():
            check(path not in pins or pins[path] == digest, "conflicting source pins")
            pins[path] = digest
    check(parameters[VARIANTS[0]]["runtime"] == parameters[VARIANTS[1]]["runtime"], "decoder policies differ between arms")
    sources = {b["path"]: b for b in freeze["sources"]}
    check(len(sources) == len(freeze["sources"]) and (set(pins)|OWN_SOURCES).issubset(sources), "missing/duplicate source bindings")
    for path, binding in sources.items():
        data = verify(binding)
        check(path not in pins or pins[path] == binding["sha256"], "compiled pin differs")
        immutable_source(root, revision, path, data)
    check(freeze["launcher"] == sources["scripts/run-pretranscription-screening.py"], "launcher binding differs")
    models = {b["path"]: b for b in freeze["models"]}
    check(len(models) == len(freeze["models"]) == 2 and set(models) == {MODEL_PATH,VAD_PATH}, "ASR/VAD pair differs")
    check(models[MODEL_PATH]["sha256"] == common.MODEL_SHA and models[MODEL_PATH]["bytes"] == 574041195 and
          models[VAD_PATH]["sha256"] == common.VAD_SHA and models[VAD_PATH]["bytes"] == 885098, "model pin differs")
    for binding in models.values():
        verify(binding, 1024*1024*1024, False)
    inputs = {b["path"]: b for b in freeze["inputs"]}
    check(len(inputs) == len(freeze["inputs"]) == 100 and set(inputs) == {c["audio"]["path"] for c in cases}, "all100 input bindings required")
    for case, source in zip(cases, manifest["cases"]):
        check(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", case["id"]), "unsafe case ID")
        check(all(case[k] == source[k] for k in ("id","language","class","split","audio")) and
              common.sha256(source["reference"].encode()) == case["referenceUtf8Sha256"], "independent input/reference changed")
        check(case["expectedOutcome"] == ("Local" if source["reference"].strip() else "Empty"), "frozen expectation differs")
        audio = case["audio"]
        check(case["language"] in ("de","en") and audio["sampleRate"] == 16000 and integer(audio["frames"],1) and audio["frames"] <= 4_800_000, "unsupported audio/language")
        binding = inputs[audio["path"]]
        check(binding["sha256"] == audio["sha256"], "audio pin mismatch")
        data = verify(binding, 10*1024*1024)
        with wave.open(io.BytesIO(data), "rb") as wav:
            check((wav.getnchannels(),wav.getsampwidth(),wav.getframerate(),wav.getnframes(),wav.getcomptype()) ==
                  (1,2,16000,audio["frames"],"NONE") and len(wav.readframes(audio["frames"]+1)) == audio["frames"]*2, "PCM header/payload differs")
    output = common.relative_path(root, freeze["outputDirectory"])
    check(Path(freeze["outputDirectory"]).parts[0] == "artifacts" and not os.path.lexists(output) and output.parent.is_dir(), "fresh ignored output directory required")
    blocks = validate_blocks(freeze, root, cases)
    by_id = {c["id"]: c for c in cases}
    for block in blocks:
        block["cases"] = [dict(by_id[c["id"]], **{k:v for k,v in c.items() if k != "id"}) for c in block["cases"]]
    return dict(root=root, output=output, blocks=blocks, parameters=parameters, parameterBindings=freeze["parameters"],
                sourceRevision=revision, model=dict(models[MODEL_PATH], id="large-v3-turbo-q5_0"),
                fingerprints=fingerprints, freezeBytes=raw, freezeSha256=common.sha256(raw))


def stop_power(child):
    # The child may exit between poll and terminate; still reap its wait status.
    for send in (child.terminate, child.kill):
        try:
            send()
        except ProcessLookupError:
            pass
        except OSError:
            # A failed signal is not proof that the child is alive or reaped.
            pass
        try:
            child.wait(timeout=5)
            return True
        except subprocess.TimeoutExpired:
            continue
        except OSError:
            return False
    return False


_ACTIVE = None
_INTERRUPTED = False


def interrupt(_signal, _frame):
    global _INTERRUPTED
    _INTERRUPTED = True
    if _ACTIVE is not None:
        raise KeyboardInterrupt()


def group_ended(pid):
    deadline = time.monotonic()+5
    while True:
        try:
            os.killpg(pid, 0)
        except ProcessLookupError:
            return True
        except OSError:
            return False
        if time.monotonic() >= deadline:
            return False
        time.sleep(.05)


def stop_owned(child):
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except OSError:
        # Preserve/reap even when signalling failed; existence is checked below.
        pass
    try:
        child.wait(timeout=5)
    except (subprocess.TimeoutExpired, OSError):
        return False
    return group_ended(child.pid)


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def launch_case(plan, case, paths):
    global _ACTIVE
    report = dict(command=case["command"], startedUtc=now(), finishedUtc=None,
                  wrapperPid=None, exitCode=None, launchError=None, watchdogExpired=False,
                  terminationConfirmed=False)
    child = None
    try:
        descriptor = os.open(paths["log"], os.O_WRONLY|os.O_CREAT|os.O_EXCL, 0o600)
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb") as log:
            child = subprocess.Popen(case["command"], cwd=plan["root"], stdout=log, stderr=log,
                    env=dict(os.environ, EVERLAST_BENCHMARK_REVISION=plan["sourceRevision"]), start_new_session=True)
            report["wrapperPid"] = child.pid
            _ACTIVE = child
            print(json.dumps({"caseId":case["id"], "wrapperPid":child.pid, "startedUtc":report["startedUtc"]}), flush=True)
            if _INTERRUPTED:
                raise KeyboardInterrupt()
            report["exitCode"] = child.wait(timeout=WATCHDOG_SECONDS)
            report["terminationConfirmed"] = group_ended(child.pid)
            if not report["terminationConfirmed"]:
                report["launchError"] = "owned process group remained after wrapper exit"
                report["terminationConfirmed"] = stop_owned(child)
    except (OSError, subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        _ACTIVE = None
        report["watchdogExpired"] = isinstance(error, subprocess.TimeoutExpired)
        report["launchError"] = "owned launch/observation failed: "+type(error).__name__
        if child is not None:
            report["terminationConfirmed"] = stop_owned(child)
            report["exitCode"] = child.returncode
        else:
            report["terminationConfirmed"] = True  # No child was created.
    finally:
        _ACTIVE = None
        report["finishedUtc"] = now()
    return report


def main(argv=None):
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze", type=Path)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--execute", action="store_true", help="Execute only after the complete immutable preflight; otherwise check only")
    args = parser.parse_args(argv)
    plan = preflight(args.freeze, args.root)
    if not args.execute:
        print(json.dumps({"preflight":True, "cases":400, "rows":800, "qualified":False}))
        return 0
    os.umask(0o077)
    signal.signal(signal.SIGINT, interrupt)
    signal.signal(signal.SIGTERM, interrupt)
    power, power_error = None, None
    try:
        power = subprocess.Popen(["/usr/bin/caffeinate", "-i", "-w", str(os.getpid())],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        power_error = "owned idle-sleep assertion could not start"
    plan["launchObservation"] = dict(startedUtc=now(), runnerPid=os.getpid(),
                                     caffeinatePid=power.pid if power is not None else None,
                                     powerError=power_error, freezeSha256=plan["freezeSha256"],
                                     qualified=False, L=None)
    def launch(case, paths):
        if power_error is not None or power is None or power.poll() is not None:
            return dict(exitCode=None, terminationConfirmed=True,
                        launchError=power_error or "owned idle-sleep assertion exited early")
        return launch_case(plan, case, paths)
    try:
        report = execute(plan, launch, lambda: power is not None and stop_power(power))
        return 0 if report["completed"] else 1
    finally:
        if power is not None and power.poll() is None:
            stop_power(power)


if __name__ == "__main__":
    sys.exit(main())
