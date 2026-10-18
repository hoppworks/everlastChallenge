#!/usr/bin/env python3
"""Frozen candidate-only FullPrefix remainder; no retry or qualification."""
import importlib.util
from pathlib import Path

_SPEC=importlib.util.spec_from_file_location("full_prefix_remainder_screen",Path(__file__).with_name("run-full-prefix-screening.py"))
screen=importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(screen)
process,common=screen.process,screen.common
check,integer=common.check,common.integer
KIND="prospective-full-prefix-remainder-v1"
VARIANT="FullPrefixCandidate"


def rows_for(case,status):
    return [dict(id=case["id"],runId="warmup" if i==0 else "measured-%03d"%i,
                 warmup=i==0,status=status,validated=False) for i in range(11)]


def validate_case(plan,case,paths):
    """Validate eleven original rows; never transform them to two-row reports."""
    rows=rows_for(case,"missing")
    pid=None
    try:
        results=common.read_json(paths["output"],common.MAX_TEXT)
        meta=common.read_json(paths["metadata"],common.MAX_METADATA)
        check(isinstance(results,list),"result array missing")
        for row,result in zip(rows,results):
            if isinstance(result,dict):
                status=result.get("status")
                row["status"]=status if status in ("ok","failed","invalid_setup","unavailable") else "invalid"
        check(len(results)==len(meta["runs"])==11,"one warmup plus ten measured rows required")
        pid=meta["processId"]
        check(integer(pid,1) and meta["hostObservation"]["powerSource"]=="AC Power","actual PID/AC unavailable")
        check(type(meta["schemaVersion"]) is int and meta["schemaVersion"]==1 and
              meta["kind"]=="paced-runtime-full-prefix-revision-v1" and meta["sourceRevision"]==plan["sourceRevision"],
              "full-prefix source/schema differs")
        check(meta["fixtureSha256"]==case["audio"]["sha256"] and type(meta["inputFrames"]) is int and
              meta["inputFrames"]==case["audio"]["frames"] and type(meta["sampleRate"]) is int and
              type(meta["channels"]) is int and (meta["sampleRate"],meta["channels"],meta["language"])==(16000,1,case["language"]),
              "input identity differs")
        check(screen.exact_json(meta["model"],{k:plan["model"][k] for k in ("id","sha256")}) and
              screen.exact_json(meta["parameters"],plan["parameters"]) and
              meta["parameterDocumentSha256"]==plan["parameterSha256"],"model/parameter binding differs")
        check(meta["fullPrefixCandidate"] is True and meta["pretranscriptionCandidate"] is None and
              meta["contextCleanup"] is None and meta["referenceSha256"] is None,"mixed experiment/reference input")
        check(type(meta["warmupRuns"]) is int and meta["warmupRuns"]==1 and
              type(meta["measuredRunsRequested"]) is int and meta["measuredRunsRequested"]==10 and
              meta["debugAssertions"] is False and meta["qualified"] is False and meta["L"] is None,
              "counts or qualification differs")
        check(meta["passed"] is True and meta["loadError"] is None and meta["cleanup"]["joined"] is True and
              meta["cleanup"]["error"] is None,"setup/runtime cleanup failed")
        check(meta["expectedOutcome"]==case["expectedOutcome"] and
              screen.exact_json(meta["engineTimingCoverage"],dict(measuredRuns=10,withActualEngineTiming=10)),
              "outcome/timing coverage differs")
        previous_job=0
        for index,(result,row) in enumerate(zip(results,meta["runs"])):
            run="warmup" if index==0 else "measured-%03d"%index
            check(set(result)=={"id","warmup","status","text"} and result["id"]==row["id"]==run and
                  result["warmup"] is row["warmup"] is (index==0),"row identity differs")
            check(result["status"]==row["status"]=="ok" and row["error"] is None and
                  type(row["session"]) is int and row["session"]==index+1 and
                  row["controllerClockMatchesCapture"] is True,"row failed/session differs")
            check(row["expectedOutcome"]==row["terminal"]==case["expectedOutcome"] and
                  row["referenceMatch"] is None and row["engineRtfUnavailableReason"] is None,
                  "terminal/reference/timing differs")
            common.validate_publisher(row)
            process.validate_feed(row,case["audio"]["frames"])
            terminal,stop=row["tTerminalOffsetNanos"],row["tStopOffsetNanos"]
            check(integer(terminal) and terminal>=stop and row["stopToTerminalNanos"]==terminal-stop,
                  "terminal clock differs")
            if case["expectedOutcome"]=="Empty":
                check(result["text"]=="" and row["tTextOffsetNanos"] is None and row["stopToTextNanos"] is None,
                      "Empty claims text or L")
            else:
                check(isinstance(result["text"],str) and bool(result["text"].strip()) and
                      len(result["text"].encode("utf-8"))<=256*1024 and row["tTextOffsetNanos"]==terminal and
                      row["stopToTextNanos"]==terminal-stop,"Local lacks complete text/clock")
            summary=screen.validate_trace(row["jobTrace"],row,case["audio"]["frames"],case["language"],True)
            jobs=row["jobTrace"]["trace"]["jobs"]
            check(jobs[0]["jobId"]>previous_job,"job IDs reset between sessions")
            previous_job=jobs[-1]["jobId"]
            rows[index].update(validated=True,session=row["session"],**summary)
        check(meta["jobTraceAfterCleanupError"] is None and
              screen.exact_json(meta["jobTraceAfterCleanup"],meta["runs"][-1]["jobTrace"]["trace"]),
              "post-join trace differs or missing")
        return dict(errors=[],rows=rows,processId=pid)
    except (OSError,ValueError,KeyError,TypeError,AttributeError,IndexError,OverflowError,ZeroDivisionError):
        # Never export an external field or hypothesis through exception text.
        return dict(errors=["missing/malformed or inconsistent full-prefix remainder report"],rows=rows,processId=pid)


def paths_for(plan,case):
    return {key:common.relative_path(plan["root"],case[key]) for key in ("output","metadata","log","completion")}


def execute(plan,launch,finish=None):
    """Serial execution; failed/partial evidence and all unavailable slots remain."""
    check(len(plan["cases"])==94,"complete94-case plan required")
    startup=process.power_snapshot()
    process.require_ac(startup) # before mkdir or any child
    output=plan["output"]
    output.mkdir(mode=0o700)
    output.chmod(0o700)
    if "freezeBytes" in plan:common.write_private_bytes(output/"freeze.json",plan["freezeBytes"])
    common.write_private(output/"launch.json",dict(plan.get("launchObservation",{}),startupPower=startup,
        runId=plan["runId"],freezeSha256=plan.get("freezeSha256"),
        powerScope="discrete startup/before-case/after-case AC observations; no continuous or lid guarantee"))
    report=dict(schemaVersion=1,kind="full-prefix-remainder-execution-v1",runId=plan["runId"],
        sourceRevision=plan["sourceRevision"],freezeSha256=plan.get("freezeSha256"),cases=[],
        completed=False,error=None,statistics=None,qualified=False,L=None)
    failure,seen_pids=None,set()
    for index,case in enumerate(plan["cases"]):
        entry=dict(id=case["id"],caseIndex=index,status="unavailable",error=failure,
                   rows=rows_for(case,"unavailable"))
        paths=paths_for(plan,case)
        if failure is None and process._INTERRUPTED:
            failure="runner interrupted; no subsequent cases launched"
            entry["error"]=failure
        if failure is None:
            before,after=process.power_snapshot(),None
            try:
                process.require_ac(before)
                common.check_unchanged(plan)
                launched=launch(case,paths)
                after=process.power_snapshot()
            except (OSError,ValueError,KeyboardInterrupt) as error:
                launched=dict(exitCode=None,terminationConfirmed=False,
                              launchError="prelaunch/launch failed: "+type(error).__name__)
            errors=[]
            if after is not None:
                try:process.require_ac(after)
                except ValueError:errors.append("post-probe AC evidence missing or battery observed")
            if launched.get("exitCode")!=0:errors.append("wrapped probe did not exit successfully")
            if launched.get("launchError"):errors.append("owned launch failed")
            if launched.get("terminationConfirmed") is not True:errors.append("owned process termination unconfirmed")
            if launched.get("terminationConfirmed") is True:
                validation=validate_case(plan,case,paths)
                raw=common.raw_bindings({"results":paths["output"],"metadata":paths["metadata"],"log":paths["log"]})
            else:
                validation=dict(errors=["owned writer unconfirmed"],rows=rows_for(case,"unavailable"),processId=None)
                raw=[dict(path=str(paths[key]),sha256=None,error="owned writer unconfirmed") for key in ("output","metadata","log")]
            errors.extend(validation["errors"])
            if any(b.get("error") or b.get("sha256") is None for b in raw):errors.append("raw binding unavailable")
            pid=validation["processId"]
            if pid is not None:
                if pid in seen_pids:errors.append("actual PID reused in frozen sequence")
                seen_pids.add(pid)
            failure="; ".join(errors) if errors else None
            entry.update(status="failed" if failure else "ok",error=failure,rows=validation["rows"],
                launch=launched,actualProbePid=pid,rawFiles=raw,beforePower=before,afterPower=after)
        common.write_private(paths["completion"],entry)
        report["cases"].append(entry)
    if finish is not None:
        try:ended=finish()
        except (OSError,ValueError):ended=False
        report["powerAssertionEnded"]=ended is True
        if ended is not True:failure=failure or "owned idle-sleep assertion termination unconfirmed"
    report.update(completed=failure is None,error=failure,rowCount=sum(len(c["rows"]) for c in report["cases"]))
    common.write_private(output/"completion.json",report)
    return report


OWN_SOURCES={"scripts/run-full-prefix-remainder.py","tests/quality/test_full_prefix_remainder.py",
    "docs/verification/full-prefix-remainder-freeze-schema.json","scripts/run-full-prefix-screening.py",
    "scripts/run-pretranscription-screening.py","scripts/run-paced-remainder.py","scripts/screening_quality/decision.py"}


def validate_decision(root,binding,fingerprints,parameters):
    """Root-owned shared admission authority; never synthesize a positive verdict."""
    specification=importlib.util.spec_from_file_location("full_prefix_remainder_decision",root/"scripts/screening_quality/decision.py")
    module=importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module.validate(root,binding,fingerprints,parameters)


def preflight(freeze_path,root):
    import io
    import os
    import re
    import stat
    import subprocess
    import uuid
    import wave
    root=Path(root).resolve(strict=True)
    freeze_path=Path(freeze_path).resolve(strict=True)
    check(root in freeze_path.parents,"freeze outside repository")
    data=common.bounded_bytes(freeze_path,common.MAX_JSON)
    freeze=common.parse_json(data)
    fields={"schemaVersion","kind","runId","sourceRevision","binary","parameters","selection","sources",
            "launcher","model","vad","inputs","outputDirectory","publisherPolicy","caseTimeoutSeconds",
            "qualityDecision","qualified","L","cases"}
    check(set(freeze)==fields and type(freeze["schemaVersion"]) is int and freeze["schemaVersion"]==1 and
          freeze["kind"]==KIND,"dedicated remainder freeze schema differs")
    revision=freeze["sourceRevision"]
    check(isinstance(revision,str) and re.fullmatch("[0-9a-f]{40}",revision),"full immutable source revision required")
    check(str(uuid.UUID(freeze["runId"]))==freeze["runId"],"canonical run UUID required")
    check(freeze["publisherPolicy"]=="timeConstrained" and type(freeze["caseTimeoutSeconds"]) is int and
          freeze["caseTimeoutSeconds"]==process.WATCHDOG_SECONDS and freeze["qualified"] is False and freeze["L"] is None,
          "TC/watchdog/unqualified scope differs")
    fingerprints={}
    own=dict(path=str(freeze_path.relative_to(root)),sha256=common.sha256(data),bytes=len(data),
             mode=oct(stat.S_IMODE(freeze_path.lstat().st_mode)))
    def verify(binding,maximum=common.MAX_JSON,read=True):
        return common.verify_binding(root,binding,fingerprints,maximum,read)
    verify(own)
    check(freeze["binary"]["path"]=="target/release/examples/paced_runtime_probe","wrong probe binary")
    verify(freeze["binary"],128*1024*1024,False)
    binary=common.relative_path(root,freeze["binary"]["path"])
    check(os.access(binary,os.X_OK),"probe not executable")
    parameters=common.parse_json(verify(freeze["parameters"],1024*1024))
    screen.validate_parameters(parameters,VARIANT)
    printed=subprocess.run([str(binary),"--print-full-prefix-candidate-parameters"],cwd=root,
        stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10,check=False)
    check(printed.returncode==0 and len(printed.stdout)<=1024*1024 and
          screen.exact_json(common.parse_json(printed.stdout),parameters),"exact compiled candidate template differs")
    pins=common.compiled_sources(parameters)
    sources={b["path"]:b for b in freeze["sources"]}
    check(len(sources)==len(freeze["sources"]) and (set(pins)|OWN_SOURCES).issubset(sources),"missing/duplicate source closure")
    for path,binding in sources.items():
        source=verify(binding)
        check(path not in pins or binding["sha256"]==pins[path],"compiled/source pin differs")
        process.immutable_source(root,revision,path,source)
    check(freeze["launcher"]==sources["scripts/run-full-prefix-remainder.py"],"launcher binding differs")
    for module,path in ((None,"scripts/run-full-prefix-remainder.py"),(screen,"scripts/run-full-prefix-screening.py"),
                        (process,"scripts/run-pretranscription-screening.py"),(common,"scripts/run-paced-remainder.py")):
        actual=common.bounded_bytes(Path(__file__ if module is None else module.__file__),common.MAX_JSON)
        check(len(actual)==sources[path]["bytes"] and common.sha256(actual)==sources[path]["sha256"],
              "executing validator differs from frozen source")
    # An explicit, separately bound actual screening decision is mandatory. The
    # shared helper also fingerprints its transitive evidence for every launch.
    validate_decision(root,freeze["qualityDecision"],fingerprints,parameters)
    model,vad=freeze["model"],freeze["vad"]
    check(model["id"]=="large-v3-turbo-q5_0" and model["sha256"]==common.MODEL_SHA and model["bytes"]==574041195 and
          vad["sha256"]==common.VAD_SHA and vad["bytes"]==885098,"model/VAD identity differs")
    check(model["path"]==process.MODEL_PATH and vad["path"]==process.VAD_PATH,"fixed model/VAD paths differ")
    check(any(m.get("id")==model["id"] and m.get("sha256")==model["sha256"] for m in parameters["models"]) and
          parameters["runtime"]["speechGate"]["model"]["sha256"]==vad["sha256"],"parameter model identity differs")
    verify(model,model["bytes"],False)
    verify(vad,vad["bytes"],False)
    check(freeze["selection"]["sha256"]==common.SELECTION_SHA,"unchanged94 selection required")
    selection=common.parse_json(verify(freeze["selection"]))
    cases=selection["cases"]
    check(len(cases)==94 and len({c["id"] for c in cases})==94 and
          sum(c["expectedOutcome"]=="Empty" for c in cases)==6,"94 cases/eighty-eight Local/six Empty required")
    manifests={}
    for original in selection["sourceManifests"]+[selection["centralManifest"]]:
        binding=dict(original,mode=oct(stat.S_IMODE(common.relative_path(root,original["path"]).lstat().st_mode)))
        manifests[binding["path"]]=common.parse_json(verify(binding))
    inputs={b["path"]:b for b in freeze["inputs"]}
    check(len(inputs)==len(freeze["inputs"])==94 and set(inputs)=={c["audio"]["path"] for c in cases},"all94 unique input bindings required")
    output=common.relative_path(root,freeze["outputDirectory"])
    check(Path(freeze["outputDirectory"]).parts[0]=="artifacts" and output.parent.is_dir() and
          not os.path.lexists(output),"fresh private artifacts namespace required")
    check(len(freeze["cases"])==94,"all94 frozen commands required")
    reserved={output/name for name in ("freeze.json","launch.json","completion.json")}
    seen=set()
    merged=[]
    for case,planned in zip(cases,freeze["cases"]):
        check(re.fullmatch("[A-Za-z0-9][A-Za-z0-9._-]{0,127}",case["id"]) and
              type(case["warmupRuns"]) is int and case["warmupRuns"]==1 and
              type(case["measuredRuns"]) is int and case["measuredRuns"]==10,"case repetition scope differs")
        originals=[c for c in manifests[case["sourceManifest"]]["cases"] if c["id"]==case["id"]]
        check(len(originals)==1,"independent original reference missing/duplicate")
        original=originals[0]
        check(all(screen.exact_json(original[k],case[k]) for k in ("language","class","split","audio")) and
              common.sha256(original["reference"].encode())==case["referenceUtf8Sha256"] and
              case["expectedOutcome"]==("Local" if original["reference"].strip() else "Empty"),"reference/selection differs")
        audio=case["audio"]
        check(audio["sampleRate"]==16000 and integer(audio["frames"],1) and audio["frames"]<=4_800_000 and
              inputs[audio["path"]]["sha256"]==audio["sha256"],"input format/frames/hash differs")
        wav_bytes=verify(inputs[audio["path"]],10*1024*1024)
        try:
            with wave.open(io.BytesIO(wav_bytes),"rb") as wav:
                check((wav.getnchannels(),wav.getsampwidth(),wav.getframerate(),wav.getnframes(),wav.getcomptype())==
                      (1,2,16000,audio["frames"],"NONE") and len(wav.readframes(audio["frames"]+1))==audio["frames"]*2,
                      "bounded PCM16 WAV differs")
        except (wave.Error,EOFError):raise ValueError("invalid bounded PCM WAV")
        check(set(planned)=={"id","command","output","metadata","log","completion"} and planned["id"]==case["id"],
              "frozen case order/fields differ")
        for key in ("output","metadata","log","completion"):
            path=common.relative_path(root,planned[key])
            check(path.parent==output and path not in seen and path not in reserved and not os.path.lexists(path),
                  "reused or nonfrozen output target")
            seen.add(path)
        check(planned["metadata"]==planned["output"]+".metadata.json","metadata sidecar target differs")
        command=["/usr/bin/time","-l",str(binary),str(root/model["path"]),"reference",str(root/audio["path"]),case["language"],
                 str(root/freeze["parameters"]["path"]),str(root/planned["output"]),"10"]
        if case["expectedOutcome"]=="Empty":command.append("--expect-empty")
        check(command==planned["command"],"exact ten-measured candidate command differs")
        merged.append(dict(case,**{k:v for k,v in planned.items() if k!="id"}))
    return dict(root=root,output=output,cases=merged,sourceRevision=revision,runId=freeze["runId"],
        model=model,parameters=parameters,parameterSha256=freeze["parameters"]["sha256"],fingerprints=fingerprints,
        freezeSha256=common.sha256(data),freezeBytes=data)


def main(argv=None):
    import argparse
    import json
    import os
    import signal
    import subprocess
    import sys
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze",type=Path)
    parser.add_argument("--root",type=Path,default=Path(__file__).resolve().parents[1])
    parser.add_argument("--execute",action="store_true")
    args=parser.parse_args(argv)
    try:plan=preflight(args.freeze,args.root)
    except (OSError,ValueError,KeyError,TypeError,AttributeError,subprocess.SubprocessError):
        print(json.dumps(dict(preflight=False,error="bounded full-prefix remainder binding validation failed")),file=sys.stderr)
        return 2
    if not args.execute:
        print(json.dumps(dict(preflight=True,cases=94,rows=1034,qualified=False,L=None,
            executionReadiness="not checked; execute requires fresh AC observations")))
        return 0
    initial=process.power_snapshot()
    try:process.require_ac(initial)
    except process.PowerAdmissionError:
        print(json.dumps(dict(executionReady=False,error="fresh AC Power required")),file=sys.stderr)
        return 2
    os.umask(0o077)
    signal.signal(signal.SIGINT,process.interrupt)
    signal.signal(signal.SIGTERM,process.interrupt)
    power,power_error=None,None
    try:
        power=subprocess.Popen(["/usr/bin/caffeinate","-i","-w",str(os.getpid())],
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    except OSError:power_error="owned idle-sleep assertion could not start"
    plan["launchObservation"]=dict(startedUtc=process.now(),runnerPid=os.getpid(),initialPower=initial,
        caffeinatePid=power.pid if power is not None else None,powerError=power_error,qualified=False,L=None)
    def launch(case,paths):
        if power_error is not None or power is None or power.poll() is not None:
            return dict(exitCode=None,terminationConfirmed=True,
                launchError=power_error or "owned idle-sleep assertion exited early")
        return process.launch_case(plan,case,paths)
    try:
        report=execute(plan,launch,lambda:power is not None and process.stop_power(power))
        return 0 if report["completed"] else 1
    except process.PowerAdmissionError:
        print(json.dumps(dict(executionReady=False,error="startup AC Power evidence unavailable")),file=sys.stderr)
        return 2
    finally:
        if power is not None and power.poll() is None:process.stop_power(power)


if __name__=="__main__":
    import sys
    sys.exit(main())
