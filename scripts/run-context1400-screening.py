#!/usr/bin/env python3
"""Two fresh Context1400 candidate blocks against explicitly historical references."""
import importlib.util
from pathlib import Path
import json
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from screening_quality import historical_reference

_SPEC = importlib.util.spec_from_file_location("context1400_full_prefix_primitives", Path(__file__).with_name("run-full-prefix-screening.py"))
full = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(full)
process, common = full.process, full.common
check, integer = common.check, common.integer


VARIANTS = {
    "p06-short-context1400-single-v1": False,
    "p06-short-context1400-full-prefix-v1": True,
}
CONTEXT_POLICY = dict(name="short-context1400", immutablePerDecoder=True, modelContextRequired=1500,
    prepared1Through320000=1400, preparedAbove320000=0, longReason="long-prepared-reference",
    noPreparedInput="no context selection and no Full call", errorRetry=False, nativeContextIntrospection=False)
KIND = "prospective-context1400-historical-reference-screening-v1"


def validate_trace(evidence, row, frames, language, full_prefix):
    summary = full.validate_trace(evidence, row, frames, language, full_prefix)
    origin = None
    for job in evidence["trace"]["jobs"]:
        receipt, prepared = job.get("contextReceipt"), job["preparedReuse"]
        check(isinstance(receipt,dict) and set(receipt) == {"session","generation","jobId","selection",
              "nativeFullAttempted","cacheOriginJobId"} and
              all(type(receipt[k]) is int and receipt[k] == job[k] for k in ("session","generation","jobId")),
              "missing or foreign context receipt")
        if prepared["outcome"] == "not-admitted":
            check(receipt["selection"] is None and receipt["nativeFullAttempted"] is False and
                  receipt["cacheOriginJobId"] is None, "context selected/native work invented after rejection")
            origin = None
            continue
        short = prepared["preparedFrames"] <= 320000
        selected = dict(policy="short-context1400", reason="short-prepared" if short else "long-prepared-reference",
            parameter=1400 if short else 0, effectiveContext=1400 if short else 1500, modelContext=1500,
            preparedFrames=prepared["preparedFrames"])
        check(full.exact_json(receipt["selection"],selected), "context policy/selection differs")
        if prepared["outcome"] == "decoded":
            check(receipt["nativeFullAttempted"] is True and type(receipt["cacheOriginJobId"]) is int and
                  receipt["cacheOriginJobId"] == job["jobId"], "context decode origin/native flag differs")
            origin = (job["jobId"],selected)
        else:
            check(receipt["nativeFullAttempted"] is False and origin is not None and
                  type(receipt["cacheOriginJobId"]) is int and receipt["cacheOriginJobId"] == prepared["originJobId"] == origin[0] and
                  full.exact_json(selected,origin[1]), "context hit lacks matching native origin")
    return summary


def execute(plan, launch, finish=None):
    """New fixed screen loop; shared owned-process/power/file primitives only."""
    check(len(plan["blocks"]) == 2 and [b["name"] for b in plan["blocks"]] == ["C1", "C2"] and all(b["variant"] == plan["variant"] for b in plan["blocks"]) and plan["variant"] in VARIANTS and
          all(len(block["cases"]) == 100 for block in plan["blocks"]), "complete two-block candidate200 plan required")
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
    report = dict(schemaVersion=1,kind="context1400-historical-reference-screening-execution-v1",screeningId=plan["screeningId"],
        sourceRevision=plan["sourceRevision"],freezeSha256=plan.get("freezeSha256"),blocks=[],completed=False,
        historicalReference=plan.get("historicalReference"),referenceEquivalence=plan.get("referenceEquivalence"),
        referencePairs=[dict(candidateBlock="C1",historicalBlock="B1"),dict(candidateBlock="C2",historicalBlock="B4")],
        newABBA=False,historicalTimingReused=False,
        qualified=False,L=None,error=None)
    failure, slot = None, 0
    for block in plan["blocks"]:
        block_report = dict(name=block["name"],variant=block["variant"],cases=[])
        for case in block["cases"]:
            slot += 1
            entry = dict(id=case["id"],globalSlot=slot,blockName=block["name"],variant=block["variant"],
                screeningId=plan["screeningId"],freezeSha256=plan.get("freezeSha256"),status="unavailable",
                error=failure,rows=[dict(row,validated=False) for row in process.canonical_rows(case,"unavailable")])
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

def validate_case(plan, block, case, paths):
    rows = [dict(row,validated=False) for row in process.canonical_rows(case, "missing")]
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
        candidate = VARIANTS[variant]
        check(variant in VARIANTS and full.exact_json(meta["parameters"],plan["parameters"]) and
              meta["parameterDocumentSha256"] == plan["parameterBinding"]["sha256"], "parameter binding differs")
        check(meta["fullPrefixCandidate"] is candidate and meta["pretranscriptionCandidate"] is None and
              meta["contextCleanup"] is None and meta["referenceSha256"] is None, "mixed context1400 experiment/reference input")
        check(type(meta["warmupRuns"]) is int and meta["warmupRuns"] == 1 and
              type(meta["measuredRunsRequested"]) is int and meta["measuredRunsRequested"] == 1 and
              meta["debugAssertions"] is False and meta["qualified"] is False and meta["L"] is None,
              "counts or qualification differs")
        check(meta["passed"] is True and meta["loadError"] is None and meta["cleanup"]["joined"] is True and
              meta["cleanup"]["error"] is None, "setup/runtime cleanup failed")
        check(meta["expectedOutcome"] == case["expectedOutcome"] and
              full.exact_json(meta["engineTimingCoverage"],{"measuredRuns":1,"withActualEngineTiming":1}), "outcome/timing coverage differs")
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
              full.exact_json(meta["jobTraceAfterCleanup"],meta["runs"][-1]["jobTrace"]["trace"]), "post-join trace differs or missing")
        return dict(errors=[],rows=rows,processId=actual_pid)
    except ValueError as error:
        return dict(errors=[str(error)],rows=rows,processId=actual_pid)
    except (OSError,KeyError,TypeError,AttributeError,IndexError,OverflowError,ZeroDivisionError):
        return dict(errors=["missing/malformed bounded full-prefix report"],rows=rows,processId=actual_pid)


def validate_parameters(document, variant, reference):
    check(variant in VARIANTS and document["configurationId"] == variant and
          document["fullPrefix"]["candidate"] is VARIANTS[variant] and "pretranscription" not in document,
          "wrong explicit context1400 parameter mode")
    check(full.exact_json(document["contextPolicy"],CONTEXT_POLICY), "context1400 policy differs")
    expected_runtime = json.loads(json.dumps(reference["runtime"]))
    expected_runtime["fullParams"]["audio_ctx"] = {"selectedPerPreparedInput":"contextPolicy"}
    check(full.exact_json(document["runtime"],expected_runtime) and document["build"]["debugAssertions"] is False,
          "context1400 changed another decoder parameter")
    expected_prefix = json.loads(json.dumps(reference["fullPrefix"]))
    expected_prefix.update(candidate=VARIANTS[variant],
        decoderPolicy="fixed ShortContext1400; prepared1..320000 selects1400, longer selects verified model default0; otherwise unchanged",
        cache="exact prepared f32LE SHA256+frames+language+immutable ContextPolicy+selected parameter; current validation/energy/VAD/padding before every lookup")
    check(full.exact_json(document["fullPrefix"],expected_prefix), "context1400 prefix/cache policy differs")
    for key in ("preprocessing","state","models"):
        check(full.exact_json(document[key],reference[key]), "context1400 changed reference preparation/state/models")
    for key,value in {"chunkMillis":20,"jitterToleranceNanos":10_000_000,
                      "maximumSleepNanos":1_000_000,"finalSpinNanos":250_000}.items():
        check(full.exact_json(document["measurement"][key],value), "input timing policy differs")
    check(document["measurement"]["mode"] == "paced-full-prefix-revision-v1" and
          document["measurement"]["publisherThread"]["policy"] == "timeConstrained", "publisher policy differs")


def validate_equivalence(root, binding, fingerprints, freeze, history):
    check(isinstance(binding,dict), "external reference-equivalence authority required")
    value = common.parse_json(common.verify_binding(root,binding,fingerprints,common.MAX_JSON,True))
    check(type(value["schemaVersion"]) is int and value["schemaVersion"] == 1 and
          value["kind"] == "context1400-reference-equivalence-v1" and value["passed"] is True and
          value["scope"] == "historical-reference-text-comparison-only" and
          value["historicalReferenceApprovalSha256"] == freeze["historicalReferenceApproval"]["sha256"] and
          value["historicalSourceRevision"] == history["source_revision"] and
          value["candidateSourceRevision"] == freeze["sourceRevision"] and value["candidateVariant"] == freeze["variant"] and
          value["candidateParametersSha256"] == freeze["parameters"]["sha256"] and
          value["manifestSha256"] == freeze["manifest"]["sha256"] and
          value["timingTransfer"] is False and value["productionApproval"] is False,
          "external reference-equivalence authority does not match candidate/history")
    check(isinstance(value["reviewEvidence"],dict) and integer(value["reviewEvidence"].get("bytes"),1),
          "reference-equivalence review evidence missing")
    common.verify_binding(root,value["reviewEvidence"],fingerprints,common.MAX_JSON)
    return binding


def validate_blocks(freeze, root, cases):
    import os
    blocks = freeze["blocks"]
    check(len(blocks) == 2 and [b["name"] for b in blocks] == ["C1","C2"] and
          all(b["variant"] == freeze["variant"] for b in blocks) and freeze["variant"] in VARIANTS,
          "two fixed same-candidate blocks required")
    output = common.relative_path(root,freeze["outputDirectory"])
    seen = {output/name for name in ("freeze.json","launch.json","completion.json")}
    binary = common.relative_path(root,freeze["binary"]["path"])
    parameters = common.relative_path(root,freeze["parameters"]["path"])
    for block in blocks:
        check(len(block["cases"]) == len(cases) == 100,"complete100 per block required")
        for entry,case in zip(block["cases"],cases):
            check(set(entry) == {"id","command","output","metadata","log","completion"} and
                  entry["id"] == case["id"],"candidate case identity/order differs")
            paths = {key:common.relative_path(root,entry[key]) for key in ("output","metadata","log","completion")}
            for path in paths.values():
                check(path.parent == output and path not in seen and not os.path.lexists(path),"output collision/nonfresh target")
                seen.add(path)
            check(paths["metadata"] == Path(str(paths["output"])+".metadata.json"),"automatic metadata path differs")
            command = ["/usr/bin/time","-l",str(binary),str(common.relative_path(root,process.MODEL_PATH)),"reference",
                       str(common.relative_path(root,case["audio"]["path"])),case["language"],str(parameters),str(paths["output"]),"1"]
            if case["expectedOutcome"] == "Empty":
                command.append("--expect-empty")
            check(entry["command"] == command,"not the fixed one-warmup/one-measured command")
    return blocks


OWN_SOURCES = {"scripts/run-context1400-screening.py","tests/quality/test_context1400_screening.py",
               "scripts/screening_quality/historical_reference.py","scripts/screening_quality/decision.py",
               "scripts/screening_quality/__init__.py","scripts/run-full-prefix-screening.py",
               "scripts/run-pretranscription-screening.py","scripts/run-paced-remainder.py"}


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
    fields = {"schemaVersion","kind","screeningId","sourceRevision","binary","selection","manifest",
              "parameters","sources","launcher","models","inputs","outputDirectory","publisherPolicy",
              "caseTimeoutSeconds","qualified","L","blocks","variant","historicalReferenceApproval","referenceEquivalenceAuthority"}
    check(set(freeze) == fields and type(freeze["schemaVersion"]) is int and freeze["schemaVersion"] == 1 and
          freeze["kind"] == KIND, "wrong dedicated context1400 freeze schema")
    check(str(uuid.UUID(freeze["screeningId"])) == freeze["screeningId"] and
          isinstance(freeze["sourceRevision"],str) and re.fullmatch("[0-9a-f]{40}",freeze["sourceRevision"]) and
          freeze["variant"] in VARIANTS, "invalid candidate source/mode/screen identity")
    check(freeze["caseTimeoutSeconds"] == process.WATCHDOG_SECONDS and freeze["publisherPolicy"] == "timeConstrained" and
          freeze["qualified"] is False and freeze["L"] is None,"scope/publisher/watchdog differs")
    freeze_path = Path(freeze_path).resolve(strict=True)
    check(root in freeze_path.parents,"freeze outside repository")
    fingerprints = {}
    common.verify_binding(root,dict(path=str(freeze_path.relative_to(root)),bytes=len(raw),sha256=common.sha256(raw),
        mode=oct(stat.S_IMODE(freeze_path.stat().st_mode))),fingerprints,common.MAX_JSON)
    def verify(binding, maximum=common.MAX_JSON, read=True):
        common.check_unchanged(dict(fingerprints=fingerprints))
        return common.verify_binding(root,binding,fingerprints,maximum,read)
    history = historical_reference.load(root,freeze["historicalReferenceApproval"],fingerprints)
    check(freeze["selection"]["sha256"] == process.SELECTION_SHA and freeze["manifest"]["sha256"] == process.MANIFEST_SHA,
          "fixed100 corpus changed")
    selection,manifest = [common.parse_json(verify(freeze[key])) for key in ("selection","manifest")]
    check(verify(freeze["manifest"]) == history["manifest_bytes"],"historical/current manifest bytes differ")
    cases = selection["cases"]
    check(manifest["unicodeVersion"] == "13.0.0" and len(cases) == len(manifest["cases"]) == 100 and
          len({c["id"] for c in cases}) == 100 and sum(c["expectedOutcome"] == "Empty" for c in cases) == 6,
          "fixed100 IDs/expectations differ")
    parameters = common.parse_json(verify(freeze["parameters"],1024*1024))
    reference = common.parse_json(verify(history["provenance"]["parameters"]["FullPrefixReference"],1024*1024))
    validate_parameters(parameters,freeze["variant"],reference)
    equivalent = validate_equivalence(root,freeze["referenceEquivalenceAuthority"],fingerprints,freeze,history)
    verify(freeze["binary"],128*1024*1024,False)
    binary = common.relative_path(root,freeze["binary"]["path"])
    check(binary.name == "paced_runtime_probe" and os.access(binary,os.X_OK),"wrong/nonexecutable probe")
    flag = "--print-short-context1400-"+("full-prefix" if VARIANTS[freeze["variant"]] else "single")+"-parameters"
    printed = subprocess.run([str(binary),flag],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10,check=False)
    check(printed.returncode == 0 and len(printed.stdout) <= 1024*1024 and
          full.exact_json(common.parse_json(printed.stdout),parameters),"parameters differ from compiled context1400 template")
    pins = common.compiled_sources(parameters)
    historical_pins = common.compiled_sources(reference)
    expected_paths = set(historical_pins) | {"src-tauri/src/transcription/context_policy.rs"}
    check(len(historical_pins) == 47 and len(expected_paths) == len(pins) == 48 and set(pins) == expected_paths,
          "complete 48-file source closure required")
    sources = {b["path"]:b for b in freeze["sources"]}
    check(len(sources) == len(freeze["sources"]) and (set(pins)|OWN_SOURCES).issubset(sources),"compiled/launcher sources missing or duplicate")
    for name,binding in sources.items():
        data = verify(binding)
        check(name not in pins or binding["sha256"] == pins[name],"compiled source differs")
        process.immutable_source(root,freeze["sourceRevision"],name,data)
    check(freeze["launcher"] == sources["scripts/run-context1400-screening.py"],"launcher binding differs")
    executing = {"scripts/run-context1400-screening.py":Path(__file__).resolve(),
                 "scripts/run-full-prefix-screening.py":Path(full.__file__).resolve(),
                 "scripts/run-pretranscription-screening.py":Path(process.__file__).resolve(),
                 "scripts/run-paced-remainder.py":Path(common.__file__).resolve(),
                 "scripts/screening_quality/historical_reference.py":Path(historical_reference.__file__).resolve(),
                 "scripts/screening_quality/decision.py":Path(historical_reference.decision.__file__).resolve()}
    for name,path in executing.items():
        check(path == (root/name).resolve(strict=True),"executing validator is outside frozen root")
    models = {b["path"]:b for b in freeze["models"]}
    check(len(models) == len(freeze["models"]) == 2 and set(models) == {process.MODEL_PATH,process.VAD_PATH} and
          models[process.MODEL_PATH]["sha256"] == common.MODEL_SHA and models[process.MODEL_PATH]["bytes"] == 574041195 and
          models[process.VAD_PATH]["sha256"] == common.VAD_SHA and models[process.VAD_PATH]["bytes"] == 885098,"model/VAD differs")
    for binding in models.values():
        verify(binding,1024*1024*1024,False)
    inputs = {b["path"]:b for b in freeze["inputs"]}
    check(len(inputs) == len(freeze["inputs"]) == 100 and set(inputs) == {c["audio"]["path"] for c in cases},"all100 inputs required")
    for case,source in zip(cases,manifest["cases"]):
        check(all(case[key] == source[key] for key in ("id","language","class","split","audio")) and
              common.sha256(source["reference"].encode()) == case["referenceUtf8Sha256"],"input/reference changed")
        check(case["expectedOutcome"] == ("Local" if source["reference"].strip() else "Empty"),"expected outcome differs")
        audio = case["audio"]
        check(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}",case["id"]) and case["language"] in ("de","en") and
              audio["sampleRate"] == 16000 and integer(audio["frames"],1) and audio["frames"] <= 4_800_000 and
              inputs[audio["path"]]["sha256"] == audio["sha256"],"audio declaration differs")
        data = verify(inputs[audio["path"]],10*1024*1024)
        with wave.open(io.BytesIO(data),"rb") as wav:
            check((wav.getnchannels(),wav.getsampwidth(),wav.getframerate(),wav.getnframes(),wav.getcomptype()) ==
                  (1,2,16000,audio["frames"],"NONE") and len(wav.readframes(audio["frames"]+1)) == audio["frames"]*2,
                  "PCM header/payload differs")
    output = common.relative_path(root,freeze["outputDirectory"])
    check(Path(freeze["outputDirectory"]).parts[0] == "artifacts" and not os.path.lexists(output) and output.parent.is_dir(),"fresh ignored output required")
    blocks = validate_blocks(freeze,root,cases)
    by_id = {c["id"]:c for c in cases}
    blocks = [dict(b,cases=[dict(by_id[e["id"]],**{k:v for k,v in e.items() if k != "id"}) for e in b["cases"]]) for b in blocks]
    common.check_unchanged(dict(fingerprints=fingerprints))
    return dict(root=root,output=output,blocks=blocks,variant=freeze["variant"],parameters=parameters,parameterBinding=freeze["parameters"],
        sourceRevision=freeze["sourceRevision"],screeningId=freeze["screeningId"],model=dict(models[process.MODEL_PATH],id="large-v3-turbo-q5_0"),
        fingerprints=fingerprints,freezeBytes=raw,freezeSha256=common.sha256(raw),
        historicalReference=dict(approval=freeze["historicalReferenceApproval"],sourceRevision=history["source_revision"],
            provenance=history["provenance"],timingReused=False),referenceEquivalence=equivalent)


def main(argv=None):
    import argparse
    import os
    import signal
    import subprocess
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze",type=Path)
    parser.add_argument("--root",type=Path,default=Path(__file__).resolve().parents[1])
    parser.add_argument("--execute",action="store_true")
    args = parser.parse_args(argv)
    try:
        plan = preflight(args.freeze,args.root)
    except (OSError,ValueError,KeyError,TypeError,IndexError,subprocess.TimeoutExpired) as error:
        print(json.dumps(dict(preflight=False,error=str(error),executionReady=False,qualified=False,L=None)),file=sys.stderr)
        return 2
    if not args.execute:
        print(json.dumps(dict(preflight=True,newCases=200,newRows=400,historicalReferenceArrays=4,newABBA=False,
            qualified=False,L=None,executionReadiness="fresh AC observations still required")))
        return 0
    try:
        process.require_ac(process.power_snapshot())
    except process.PowerAdmissionError:
        print(json.dumps(dict(executionReady=False,error="fresh AC Power required")),file=sys.stderr)
        return 2
    os.umask(0o077)
    signal.signal(signal.SIGINT,process.interrupt)
    signal.signal(signal.SIGTERM,process.interrupt)
    power,power_error = None,None
    try:
        power = subprocess.Popen(["/usr/bin/caffeinate","-i","-w",str(os.getpid())],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    except OSError:
        power_error = "owned idle-sleep assertion could not start"
    plan["launchObservation"] = dict(startedUtc=process.now(),runnerPid=os.getpid(),
        caffeinatePid=power.pid if power is not None else None,powerError=power_error,qualified=False,L=None)
    def launch(case,paths):
        if power_error is not None or power is None or power.poll() is not None:
            return dict(exitCode=None,terminationConfirmed=True,launchError=power_error or "owned idle-sleep assertion exited early")
        return process.launch_case(plan,case,paths)
    try:
        report = execute(plan,launch,lambda:power is not None and process.stop_power(power))
        return 0 if report["completed"] else 1
    except process.PowerAdmissionError:
        print(json.dumps(dict(executionReady=False,error="startup AC Power evidence unavailable")),file=sys.stderr)
        return 2
    finally:
        if power is not None and power.poll() is None:
            process.stop_power(power)


if __name__ == "__main__":
    sys.exit(main())
