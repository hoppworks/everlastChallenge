#!/usr/bin/env python3
"""Frozen full-prefix pipeline/quality screening, never latency qualification."""
import importlib.util
import math
import json
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location("full_prefix_process_routines", Path(__file__).with_name("run-pretranscription-screening.py"))
process = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(process)
common = process.common
check, integer = common.check, common.integer


def exact_json(left, right):
    """Typed JSON equality: Python's True == 1 must not weaken bindings."""
    def canonical(value):
        return json.dumps(value,sort_keys=True,separators=(",",":"),allow_nan=False)
    return canonical(left) == canonical(right)


def validate_trace(evidence, row, frames, language, candidate):
    """Independent overlap/receipt proof, never the disjoint pretrans validator."""
    check(integer(frames,1) and frames <= 4_800_000 and type(candidate) is bool,
          "invalid full-prefix fixture/variant")
    trace, session = evidence["trace"], row["session"]
    check(integer(session, 1) and evidence["error"] is None and trace["error"] is None,
          "full-prefix trace failed or session missing")
    check(trace["enabled"] is True and type(trace["session"]) is int and trace["session"] == session and
          trace["maximumJobs"] == 601 and type(trace["overflowEvents"]) is int and trace["overflowEvents"] == 0 and
          trace["firstOverflowJob"] is None, "full-prefix trace capacity/session differs")
    jobs, proofs = trace["jobs"], evidence["inputProofs"]
    check(isinstance(jobs, list) and 1 <= len(jobs) <= 601 and isinstance(proofs, list) and
          len(proofs) == len(jobs) and (candidate or len(jobs) == 1), "full-prefix job/proof count differs")
    publications = row["feed"]["publications"]
    previous_id = previous_cut = processed = worker_span = 0
    previous_adopt = None
    entry = None
    terminal = row["tTerminalOffsetNanos"]
    check(integer(terminal), "missing controller terminal time")
    for index, (job, proof) in enumerate(zip(jobs, proofs)):
        final = index == len(jobs)-1
        jid, end = job["jobId"], job["timelineEnd"]
        check(type(job["session"]) is int and job["session"] == session and
              type(job["generation"]) is int and job["generation"] == 1 and job["language"] == language,
              "full-prefix job identity differs")
        check(integer(jid, 1) and jid > previous_id and integer(end) and end <= frames and
              all(integer(job[key]) for key in ("timelineStart", "storageStart", "storageEnd")) and
              job["timelineStart"] == job["storageStart"] == 0 and
              type(job["inputFrames"]) is int and job["storageEnd"] == job["inputFrames"] == end and
              job["audioReleasedAtSubmit"] is final and (end == frames if final else end > previous_cut),
              "prefix order, storage or final whole-recording range differs")
        times = [job[k] for k in ("hostSubmitOffsetNanos", "serviceAcceptedOffsetNanos",
                 "workerBeforeDecodeOffsetNanos", "workerAfterDecodeOffsetNanos", "workerSettledOffsetNanos",
                 "completionObservedOffsetNanos", "resultAdoptedOffsetNanos")]
        check(all(integer(at) for at in times) and times == sorted(times) and times[-1] <= terminal and
              (previous_adopt is None or previous_adopt <= times[0]) and
              job["workerOutcome"] == "decoded-ok" and job["adoption"] == "accepted",
              "missing or unordered full-prefix completion/adoption")
        required = end if final else end + 4800
        check(required <= frames and type(job["requiredStableFrames"]) is int and
              job["requiredStableFrames"] == required, "independent prefix availability guard differs")
        through = 0
        for publication in publications:
            if publication["afterPublishNanos"] > times[0]:
                break
            check(publication["startFrame"] == through, "complete-publication input gap")
            through = publication["endFrame"]
        check(all(integer(proof[key]) for key in ("jobId","requiredStableFrames","publishedThroughFramesAtAdmission")) and
              through >= required and proof == dict(jobId=jid, requiredStableFrames=required,
              publishedThroughFramesAtAdmission=through), "prefix precedes complete publication or proof differs")
        receipt = job["preparedReuse"]
        check(isinstance(receipt, dict) and type(receipt["session"]) is int and type(receipt["generation"]) is int and
              type(receipt["jobId"]) is int and (receipt["session"],receipt["generation"],receipt["jobId"]) ==
              (session,1,jid), "missing or foreign prepared-reuse receipt")
        outcome = receipt["outcome"]
        if outcome == "not-admitted":
            check(receipt["preparedFrames"] is None and receipt["preparedPcmSha256"] is None and
                  receipt["originJobId"] is None and receipt["decoderInvoked"] is False and
                  receipt["nativeFullAttempted"] is False, "nonadmission invents preparation/native work")
            entry = None
        else:
            digest, prepared = receipt["preparedPcmSha256"], receipt["preparedFrames"]
            check(integer(prepared, 16000) and prepared <= max(end,16000) and isinstance(digest,list) and
                  len(digest) == 32 and all(integer(byte) and byte <= 255 for byte in digest),
                  "prepared PCM bound/hash differs")
            if outcome == "decoded":
                check(type(receipt["originJobId"]) is int and receipt["originJobId"] == jid and
                      receipt["decoderInvoked"] is True and receipt["nativeFullAttempted"] is True,
                      "successful native decode receipt lacks actual native attempt")
                entry = (jid, prepared, digest)
            else:
                check(outcome == "hit" and candidate and type(receipt["originJobId"]) is int and receipt["decoderInvoked"] is False and
                      receipt["nativeFullAttempted"] is False and entry is not None and
                      (receipt["originJobId"],prepared,digest) == entry and entry[0] < jid,
                      "hit lacks the last live native origin in the current scope")
        processed += end
        worker_span += times[3]-times[2]
        previous_id, previous_cut, previous_adopt = jid, end, times[-1]
    check(type(evidence["processedFramesSum"]) is int and evidence["processedFramesSum"] == processed and
          type(row["engineInputFrames"]) is int and row["engineInputFrames"] == processed and
          type(row["sessionRtfRecordingFrames"]) is int and row["sessionRtfRecordingFrames"] == frames,
          "overlapping processed sum/unique recording denominator differs")
    check(type(evidence["checkedWorkerSpanSumNanos"]) is int and evidence["checkedWorkerSpanSumNanos"] == worker_span and
          type(evidence["finalAdoptionOffsetNanos"]) is int and evidence["finalAdoptionOffsetNanos"] == previous_adopt,
          "exported enclosing spans/final adoption differ from independent reconstruction")
    elapsed, rtf = row["engineElapsedNanos"], row["engineRtf"]
    check(integer(elapsed) and elapsed <= worker_span, "decoder elapsed exceeds enclosing worker spans")
    check(type(rtf) in (int,float) and math.isfinite(rtf) and
          math.isclose(rtf, elapsed*16000/frames/1e9, rel_tol=1e-12, abs_tol=1e-15), "session RTF differs")
    process.validate_retired(trace["retiredJob"], session)
    return dict(jobCount=len(jobs), processedFramesSum=processed, recordingFrames=frames,
                workerSpanNanos=worker_span, engineElapsedNanos=elapsed, engineRtf=rtf)


VARIANTS = ("FullPrefixReference", "FullPrefixCandidate", "FullPrefixCandidate", "FullPrefixReference")
KIND = "prospective-full-prefix-screening-v1"
OWN_SOURCES = {"scripts/run-full-prefix-screening.py", "tests/quality/test_full_prefix_screening.py",
               "docs/verification/full-prefix-screening-freeze-schema.json",
               "scripts/run-pretranscription-screening.py", "scripts/run-paced-remainder.py"}


def execute(plan, launch, finish=None):
    """New fixed screen loop; shared owned-process/power/file primitives only."""
    check(len(plan["blocks"]) == 4 and tuple(b["variant"] for b in plan["blocks"]) == VARIANTS and
          all(len(block["cases"]) == 100 for block in plan["blocks"]), "complete ABBA400 plan required")
    startup = process.power_snapshot()
    process.require_ac(startup)
    output = plan["output"]
    output.mkdir(mode=0o700)
    output.chmod(0o700)
    if "freezeBytes" in plan:
        common.write_private_bytes(output/"freeze.json",plan["freezeBytes"])
    common.write_private(output/"launch.json",dict(plan.get("launchObservation",{}),startupPower=startup,
        screeningId=plan["screeningId"],freezeSha256=plan.get("freezeSha256"),
        powerScope="discrete startup/before-case/after-case AC observations; no continuous or lid guarantee"))
    report = dict(schemaVersion=1,kind="full-prefix-screening-execution-v1",screeningId=plan["screeningId"],
        sourceRevision=plan["sourceRevision"],freezeSha256=plan.get("freezeSha256"),blocks=[],completed=False,
        qualified=False,L=None,error=None)
    failure, slot = None, 0
    for block in plan["blocks"]:
        block_report = dict(name=block["name"],variant=block["variant"],cases=[])
        for case in block["cases"]:
            slot += 1
            entry = dict(id=case["id"],globalSlot=slot,blockName=block["name"],variant=block["variant"],
                screeningId=plan["screeningId"],freezeSha256=plan.get("freezeSha256"),status="unavailable",
                error=failure,rows=process.canonical_rows(case,"unavailable"))
            if failure is None and process._INTERRUPTED:
                failure = "runner interrupted; no subsequent cases launched"
                entry["error"] = failure
            paths = process.paths_for(plan,case)
            if failure is None:
                before, after = process.power_snapshot(), None
                try:
                    process.require_ac(before)
                    common.check_unchanged(plan)
                    launched = launch(case,paths)
                    after = process.power_snapshot()
                except (OSError,ValueError,KeyboardInterrupt) as error:
                    launched = dict(exitCode=None,terminationConfirmed=False,
                        launchError="prelaunch/launch failed: "+type(error).__name__)
                errors = []
                if after is not None:
                    try:
                        process.require_ac(after)
                    except ValueError:
                        errors.append("post-probe AC Power evidence missing or battery observed")
                if launched.get("exitCode") != 0:
                    errors.append("wrapped probe did not exit successfully")
                if launched.get("terminationConfirmed") is not True:
                    errors.append("owned process termination unconfirmed")
                if launched.get("launchError"):
                    errors.append(launched["launchError"])
                validation = validate_case(plan,block,case,paths)
                errors.extend(validation["errors"])
                if launched.get("terminationConfirmed") is True:
                    raw = common.raw_bindings(dict(results=paths["output"],metadata=paths["metadata"],log=paths["log"]))
                else:
                    # A writer whose ownership could not be closed may still
                    # grow a file: retain its path, never hash an unbounded EOF.
                    raw = [dict(path=str(paths[key]),sha256=None,error="owned writer termination unconfirmed")
                           for key in ("output","metadata","log")]
                if any(binding.get("error") or binding.get("sha256") is None for binding in raw):
                    errors.append("raw artifact missing, changed, oversized or not private")
                actual_pid = validation.get("processId")
                if actual_pid is not None:
                    seen = plan.setdefault("seenPids",set())
                    if actual_pid in seen:
                        errors.append("actual probe PID reused in frozen sequence")
                    seen.add(actual_pid)
                failure = "; ".join(errors) if errors else None
                entry.update(status="failed" if failure else "ok",error=failure,rows=validation["rows"],
                    launch=launched,actualProbePid=actual_pid,rawFiles=raw,beforePower=before,afterPower=after)
            common.write_private(paths["completion"],entry)
            block_report["cases"].append(entry)
        report["blocks"].append(block_report)
    if finish is not None:
        try:
            ended = finish()
        except (OSError,ValueError):
            ended = False
        report["powerAssertionEnded"] = ended is True
        if ended is not True:
            failure = failure or "owned idle-sleep assertion termination unconfirmed"
    report.update(completed=failure is None,error=failure,
                  rowCount=sum(len(case["rows"]) for block in report["blocks"] for case in block["cases"]))
    common.write_private(output/"completion.json",report)
    return report


def validate_parameters(document, variant):
    candidate = variant == "FullPrefixCandidate"
    check(variant in VARIANTS and document["configurationId"] ==
          "p06-full-prefix-" + ("candidate" if candidate else "reference") + "-v1" and
          document["fullPrefix"]["candidate"] is candidate and "pretranscription" not in document,
          "wrong explicit full-prefix parameter mode")
    check(document["build"]["debugAssertions"] is False and
          document["runtime"]["context"]["flash_attn"] is False and
          document["runtime"]["fullParams"]["audio_ctx"] == 0 and
          document["runtime"]["fullParams"]["no_timestamps"] is False and
          document["runtime"]["fullParams"]["suppress_nst"] is False, "full reference decoder policy differs")
    for key, value in {"windowFrames":320,"startRms":0.00005,"endRms":0.000025,"prerollFrames":2400,
                       "tailFrames":3200,"confirmationFrames":8000,"maximumJobs":601}.items():
        check(document["fullPrefix"][key] == value, "frozen full-prefix segmentation policy differs")
    for key, value in {"chunkMillis":20,"jitterToleranceNanos":10_000_000,
                       "maximumSleepNanos":1_000_000,"finalSpinNanos":250_000}.items():
        check(document["measurement"][key] == value, "frozen input timing policy differs")
    check(document["measurement"]["mode"] == "paced-full-prefix-revision-v1" and
          document["measurement"]["publisherThread"]["policy"] == "timeConstrained" and
          document["fullPrefix"]["qualification"] is False, "full-prefix TC/screen scope differs")


def validate_case(plan, block, case, paths):
    rows = process.canonical_rows(case, "missing")
    actual_pid = None
    try:
        results = common.read_json(paths["output"], common.MAX_TEXT)
        meta = common.read_json(paths["metadata"], common.MAX_METADATA)
        check(isinstance(results,list), "result array missing")
        for index, result in enumerate(results[:2]):
            if isinstance(result,dict):
                rows[index]["status"] = result.get("status","invalid")
        check(len(results) == len(meta["runs"]) == 2, "one warmup plus one measured row required")
        actual_pid = meta["processId"]
        check(integer(actual_pid,1) and meta["hostObservation"]["powerSource"] == "AC Power", "actual PID/AC unavailable")
        check(type(meta["schemaVersion"]) is int and meta["schemaVersion"] == 1 and meta["kind"] == "paced-runtime-full-prefix-revision-v1" and
              meta["sourceRevision"] == plan["sourceRevision"], "full-prefix source/schema differs")
        check(meta["fixtureSha256"] == case["audio"]["sha256"] and
              type(meta["inputFrames"]) is int and meta["inputFrames"] == case["audio"]["frames"] and
              type(meta["sampleRate"]) is int and type(meta["channels"]) is int and
              (meta["sampleRate"],meta["channels"],meta["language"]) == (16000,1,case["language"]), "input identity differs")
        check(meta["model"] == {"id":plan["model"]["id"],"sha256":plan["model"]["sha256"]}, "model identity differs")
        variant = block["variant"]
        candidate = variant == "FullPrefixCandidate"
        check(variant in VARIANTS and exact_json(meta["parameters"],plan["parameters"][variant]) and
              meta["parameterDocumentSha256"] == plan["parameterBindings"][variant]["sha256"], "parameter binding differs")
        check(meta["fullPrefixCandidate"] is candidate and meta["pretranscriptionCandidate"] is None and
              meta["contextCleanup"] is None and meta["referenceSha256"] is None, "mixed experiment/reference input")
        check(type(meta["warmupRuns"]) is int and meta["warmupRuns"] == 1 and
              type(meta["measuredRunsRequested"]) is int and meta["measuredRunsRequested"] == 1 and
              meta["debugAssertions"] is False and meta["qualified"] is False and meta["L"] is None,
              "counts or qualification differs")
        check(meta["passed"] is True and meta["loadError"] is None and meta["cleanup"]["joined"] is True and
              meta["cleanup"]["error"] is None, "setup/runtime cleanup failed")
        check(meta["expectedOutcome"] == case["expectedOutcome"] and
              exact_json(meta["engineTimingCoverage"],{"measuredRuns":1,"withActualEngineTiming":1}), "outcome/timing coverage differs")
        previous_job = 0
        for index, (result,row) in enumerate(zip(results,meta["runs"])):
            run = "warmup" if index == 0 else "measured-001"
            check(set(result) == {"id","warmup","status","text"} and result["id"] == row["id"] == run and
                  result["warmup"] is row["warmup"] is (index == 0), "row identity differs")
            check(result["status"] == row["status"] == "ok" and row["error"] is None and
                  type(row["session"]) is int and row["session"] == index+1 and
                  row["controllerClockMatchesCapture"] is True, "row failed/session differs")
            check(row["expectedOutcome"] == row["terminal"] == case["expectedOutcome"] and
                  row["referenceMatch"] is None and row["engineRtfUnavailableReason"] is None, "terminal/reference/timing differs")
            common.validate_publisher(row)
            process.validate_feed(row,case["audio"]["frames"])
            terminal, stopped = row["tTerminalOffsetNanos"], row["tStopOffsetNanos"]
            check(integer(terminal) and terminal >= stopped and row["stopToTerminalNanos"] == terminal-stopped,
                  "terminal clock differs")
            if case["expectedOutcome"] == "Empty":
                check(result["text"] == "" and row["tTextOffsetNanos"] is None and row["stopToTextNanos"] is None,
                      "Empty claims text or L")
            else:
                check(isinstance(result["text"],str) and bool(result["text"].strip()) and
                      row["tTextOffsetNanos"] == terminal and row["stopToTextNanos"] == terminal-stopped,
                      "Local lacks complete text/clock")
            summary = validate_trace(row["jobTrace"],row,case["audio"]["frames"],case["language"],candidate)
            jobs = row["jobTrace"]["trace"]["jobs"]
            check(jobs[0]["jobId"] > previous_job, "job IDs reset between fresh sessions")
            previous_job = jobs[-1]["jobId"]
            rows[index].update(validated=True,session=row["session"],**summary)
        check(meta["jobTraceAfterCleanupError"] is None and
              exact_json(meta["jobTraceAfterCleanup"],meta["runs"][-1]["jobTrace"]["trace"]), "post-join trace differs or missing")
        return dict(errors=[],rows=rows,processId=actual_pid)
    except ValueError as error:
        return dict(errors=[str(error)],rows=rows,processId=actual_pid)
    except (OSError,KeyError,TypeError,AttributeError,IndexError,OverflowError,ZeroDivisionError):
        return dict(errors=["missing/malformed bounded full-prefix report"],rows=rows,processId=actual_pid)


def validate_blocks(freeze, root, cases):
    import os
    blocks = freeze["blocks"]
    check(isinstance(blocks,list) and len(blocks) == 4 and tuple(b["variant"] for b in blocks) == VARIANTS,
          "full-prefix ABBA order differs")
    check(len({b["name"] for b in blocks}) == 4, "duplicate block identity")
    output, seen = common.relative_path(root,freeze["outputDirectory"]), set()
    reserved = {output/name for name in ("freeze.json","launch.json","completion.json")}
    binary = common.relative_path(root,freeze["binary"]["path"])
    for block in blocks:
        check(len(block["cases"]) == 100, "each block requires all100 fixed cases")
        parameter = common.relative_path(root,freeze["parameters"][block["variant"]]["path"])
        for entry, case in zip(block["cases"],cases):
            check(set(entry) == {"id","command","output","metadata","log","completion"} and
                  entry["id"] == case["id"], "case identity/order differs")
            paths = {key:common.relative_path(root,entry[key]) for key in ("output","metadata","log","completion")}
            for path in paths.values():
                check(path.parent == output and path not in seen and path not in reserved and not os.path.lexists(path),
                      "output occupied, duplicated or outside new directory")
                seen.add(path)
            check(paths["metadata"] == Path(str(paths["output"])+".metadata.json"), "automatic metadata target differs")
            command = ["/usr/bin/time","-l",str(binary),str(root/process.MODEL_PATH),"reference",
                       str(common.relative_path(root,case["audio"]["path"])),case["language"],str(parameter),str(paths["output"]),"1"]
            if case["expectedOutcome"] == "Empty":
                command.append("--expect-empty")
            check(entry["command"] == command, "not the frozen one-warmup/one-measured command")
    return blocks


def preflight(freeze_path, root):
    import io
    import os
    import re
    import stat
    import subprocess
    import uuid
    import wave
    root = Path(root).resolve(strict=True)
    raw = common.bounded_bytes(freeze_path,common.MAX_JSON)
    freeze = common.parse_json(raw)
    check(set(freeze) == {"schemaVersion","kind","screeningId","sourceRevision","binary","selection","manifest",
                         "parameters","sources","launcher","models","inputs","outputDirectory","publisherPolicy",
                         "caseTimeoutSeconds","qualified","L","blocks"}, "freeze fields differ from dedicated schema")
    check(type(freeze["schemaVersion"]) is int and freeze["schemaVersion"] == 1 and freeze["kind"] == KIND, "wrong full-prefix freeze schema")
    check(str(uuid.UUID(freeze["screeningId"])) == freeze["screeningId"], "canonical screening UUID required")
    revision = freeze["sourceRevision"]
    check(isinstance(revision,str) and re.fullmatch("[0-9a-f]{40}",revision), "full immutable source revision required")
    check(freeze["caseTimeoutSeconds"] == process.WATCHDOG_SECONDS and freeze["publisherPolicy"] == "timeConstrained" and
          freeze["qualified"] is False and freeze["L"] is None, "watchdog/publisher/screen scope differs")
    check(freeze["binary"]["path"] == "target/release/examples/paced_runtime_probe", "wrong probe binary")
    check(freeze["selection"]["sha256"] == process.SELECTION_SHA and freeze["manifest"]["sha256"] == process.MANIFEST_SHA,
          "fixed100 corpus changed")
    fingerprints = {}
    freeze_path = Path(freeze_path).resolve(strict=True)
    check(root in freeze_path.parents, "freeze outside repository")
    binding = dict(path=str(freeze_path.relative_to(root)),sha256=common.sha256(raw),bytes=len(raw),
                   mode=oct(stat.S_IMODE(freeze_path.stat().st_mode)))
    common.verify_binding(root,binding,fingerprints,common.MAX_JSON)
    def verify(value, maximum=common.MAX_JSON, read=True):
        return common.verify_binding(root,value,fingerprints,maximum,read)
    verify(freeze["binary"],128*1024*1024,False)
    check(os.access(root/freeze["binary"]["path"],os.X_OK), "binary not executable")
    selection, manifest = [common.parse_json(verify(freeze[key])) for key in ("selection","manifest")]
    cases = selection["cases"]
    check(manifest["unicodeVersion"] == "13.0.0" and len(cases) == len(manifest["cases"]) == 100 and
          len({case["id"] for case in cases}) == 100 and sum(c["expectedOutcome"] == "Empty" for c in cases) == 6,
          "fixed100 membership/expectation/Unicode differs")
    check(set(freeze["parameters"]) == set(VARIANTS), "exact full-prefix arms required")
    parameters, pins = {}, {}
    for variant,binding in freeze["parameters"].items():
        document = common.parse_json(verify(binding,1024*1024))
        validate_parameters(document,variant)
        flag = "--print-full-prefix-" + ("candidate" if variant == "FullPrefixCandidate" else "reference") + "-parameters"
        printed = subprocess.run([str(root/freeze["binary"]["path"]),flag],cwd=root,
                   stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10,check=False)
        check(printed.returncode == 0 and len(printed.stdout) <= 1024*1024 and
              exact_json(common.parse_json(printed.stdout),document), "parameters differ from actual compiled full-prefix template")
        parameters[variant] = document
        for path,digest in common.compiled_sources(document).items():
            check(path not in pins or pins[path] == digest, "conflicting compiled source pins")
            pins[path] = digest
    check(exact_json(parameters[VARIANTS[0]]["runtime"],parameters[VARIANTS[1]]["runtime"]), "decoder config differs between arms")
    sources = {b["path"]:b for b in freeze["sources"]}
    check(len(sources) == len(freeze["sources"]) and (set(pins)|OWN_SOURCES).issubset(sources),
          "missing/duplicate source bindings")
    for path,binding in sources.items():
        data = verify(binding)
        check(path not in pins or pins[path] == binding["sha256"], "compiled source differs")
        process.immutable_source(root,revision,path,data)
    check(freeze["launcher"] == sources["scripts/run-full-prefix-screening.py"], "new launcher binding differs")
    models = {b["path"]:b for b in freeze["models"]}
    check(len(models) == len(freeze["models"]) == 2 and set(models) == {process.MODEL_PATH,process.VAD_PATH}, "ASR/VAD pair differs")
    check(models[process.MODEL_PATH]["sha256"] == common.MODEL_SHA and models[process.MODEL_PATH]["bytes"] == 574041195 and
          models[process.VAD_PATH]["sha256"] == common.VAD_SHA and models[process.VAD_PATH]["bytes"] == 885098,
          "model pins differ")
    for binding in models.values():
        verify(binding,1024*1024*1024,False)
    inputs = {b["path"]:b for b in freeze["inputs"]}
    check(len(inputs) == len(freeze["inputs"]) == 100 and set(inputs) == {c["audio"]["path"] for c in cases},
          "all100 independent input bindings required")
    for case,source in zip(cases,manifest["cases"]):
        check(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}",case["id"]), "unsafe case identity")
        check(all(case[key] == source[key] for key in ("id","language","class","split","audio")) and
              common.sha256(source["reference"].encode()) == case["referenceUtf8Sha256"], "input/reference changed")
        check(case["expectedOutcome"] == ("Local" if source["reference"].strip() else "Empty"), "expectation differs")
        audio = case["audio"]
        check(case["language"] in ("de","en") and audio["sampleRate"] == 16000 and integer(audio["frames"],1) and
              audio["frames"] <= 4_800_000 and inputs[audio["path"]]["sha256"] == audio["sha256"], "audio declaration differs")
        data = verify(inputs[audio["path"]],10*1024*1024)
        with wave.open(io.BytesIO(data),"rb") as wav:
            check((wav.getnchannels(),wav.getsampwidth(),wav.getframerate(),wav.getnframes(),wav.getcomptype()) ==
                  (1,2,16000,audio["frames"],"NONE") and len(wav.readframes(audio["frames"]+1)) == audio["frames"]*2,
                  "PCM header/payload differs")
    output = common.relative_path(root,freeze["outputDirectory"])
    check(Path(freeze["outputDirectory"]).parts[0] == "artifacts" and not os.path.lexists(output) and output.parent.is_dir(),
          "fresh ignored output directory required")
    blocks = validate_blocks(freeze,root,cases)
    by_id = {case["id"]:case for case in cases}
    blocks = [dict(block,cases=[dict(by_id[entry["id"]],**{k:v for k,v in entry.items() if k != "id"})
                               for entry in block["cases"]]) for block in blocks]
    return dict(root=root,output=output,blocks=blocks,parameters=parameters,parameterBindings=freeze["parameters"],
                sourceRevision=revision,screeningId=freeze["screeningId"],model=dict(models[process.MODEL_PATH],id="large-v3-turbo-q5_0"),
                fingerprints=fingerprints,freezeBytes=raw,freezeSha256=common.sha256(raw))


def main(argv=None):
    import argparse
    import json
    import os
    import signal
    import subprocess
    import sys
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze",type=Path)
    parser.add_argument("--root",type=Path,default=Path(__file__).resolve().parents[1])
    parser.add_argument("--execute",action="store_true")
    args = parser.parse_args(argv)
    plan = preflight(args.freeze,args.root)
    if not args.execute:
        print(json.dumps(dict(preflight=True,cases=400,rows=800,qualified=False,L=None,
            executionReadiness="not checked; execute requires fresh AC observations")))
        return 0
    initial = process.power_snapshot()
    try:
        process.require_ac(initial)
    except process.PowerAdmissionError:
        print(json.dumps(dict(executionReady=False,error="fresh AC Power required")),file=sys.stderr)
        return 2
    os.umask(0o077)
    signal.signal(signal.SIGINT,process.interrupt)
    signal.signal(signal.SIGTERM,process.interrupt)
    power, power_error = None, None
    try:
        power = subprocess.Popen(["/usr/bin/caffeinate","-i","-w",str(os.getpid())],
                    stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    except OSError:
        power_error = "owned idle-sleep assertion could not start"
    plan["launchObservation"] = dict(startedUtc=process.now(),runnerPid=os.getpid(),initialPower=initial,
        caffeinatePid=power.pid if power is not None else None,powerError=power_error,qualified=False,L=None)
    def launch(case,paths):
        if power_error is not None or power is None or power.poll() is not None:
            return dict(exitCode=None,terminationConfirmed=True,
                        launchError=power_error or "owned idle-sleep assertion exited early")
        return process.launch_case(plan,case,paths)
    try:
        report = execute(plan,launch,lambda: power is not None and process.stop_power(power))
        return 0 if report["completed"] else 1
    except process.PowerAdmissionError:
        print(json.dumps(dict(executionReady=False,error="startup AC Power evidence unavailable")),file=sys.stderr)
        return 2
    finally:
        if power is not None and power.poll() is None:
            process.stop_power(power)


if __name__ == "__main__":
    import sys
    sys.exit(main())
