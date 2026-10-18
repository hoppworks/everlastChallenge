#!/usr/bin/env python3
"""Two indivisible, prospective central pairs. Default CLI is preflight only."""
import argparse
import copy
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import signal
import stat
import subprocess

_SPEC = importlib.util.spec_from_file_location("central_screening", Path(__file__).with_name("run-pretranscription-screening.py"))
screen = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(screen)
common = screen.common
check, integer = common.check, common.integer
PAIRS = {"ref-cand": ("reference", "candidate"), "cand-ref": ("candidate", "reference")}
IDS = {v: "p06-six-case-pretranscription-"+v+"-v1" for v in ("reference", "candidate")}
OWN = {"scripts/run-pretranscription-central.py", "tests/quality/test_pretranscription_central.py",
       "docs/verification/pretranscription-central-freeze-schema.md",
       "scripts/run-paced-remainder.py", "scripts/run-pretranscription-screening.py"}


def canonical_cases(document):
    check(document["schemaVersion"] == 1 and document["unicodeVersion"] == "13.0.0", "central manifest schema differs")
    cases = document["cases"]
    check(isinstance(cases, list) and len(cases) == 6, "exactly six central cases required")
    for c in cases:
        a = c["audio"]
        check(c["language"] in ("de", "en") and c["class"] == "clean" and c["split"] == "acceptance" and
              isinstance(c["reference"], str) and 0 < len(c["reference"].encode()) <= 256*1024 and
              bool(c["reference"].strip()) and a["sampleRate"] == 16000 and
              type(a["frames"]) is int and a["frames"] in (80000, 160000, 240000), "invalid central case")
    check(len({c["id"] for c in cases}) == len({c["audio"]["path"] for c in cases}) ==
          len({c["audio"]["sha256"] for c in cases}) == 6, "central identities/assets reused")
    ordered = sorted(cases, key=lambda c: (c["language"], c["audio"]["frames"]))
    check([(c["language"], c["audio"]["frames"]) for c in ordered] ==
          [(lang, sec*16000) for lang in ("de","en") for sec in (5,10,15)], "central slots differ")
    return ordered


def slots(cases):
    def one(index, run):
        c = cases[index]
        return dict(id=("warmup-"+c["id"]) if run == 0 else ("run-"+c["id"]+"-%02d" % run),
            fixtureIndex=index, fixtureId=c["id"], fixtureSha256=c["audio"]["sha256"],
            language=c["language"], durationSeconds=c["audio"]["frames"]//16000, warmup=run == 0, runIndex=run)
    return [one(0, 0)] + [one(i, r) for i in range(6) for r in range(1,31)]


def validate_parameters(parameters):
    check(set(parameters) == {"reference","candidate"}, "both exact central parameter variants required")
    normalized = []
    for variant, document in parameters.items():
        candidate = variant == "candidate"
        check(document["configurationId"] == IDS[variant] and document["build"]["debugAssertions"] is False and
              document["pretranscription"]["candidate"] is candidate, "wrong central variant/release flag")
        m, full = document["measurement"], document["runtime"]["fullParams"]
        check(m["publisherThread"]["policy"] == "timeConstrained" and
              full["audio_ctx"] == 0 and full["no_timestamps"] is False and full["suppress_nst"] is False and
              document["runtime"]["context"]["flash_attn"] is False, "mixed decoder/publisher experiment")
        for key, value in dict(warmupRuns=1, measuredRunsPerCase=30, totalRows=181, chunkMillis=20,
                               jitterToleranceNanos=10_000_000, maximumSleepNanos=1_000_000, finalSpinNanos=250_000).items():
            check(m[key] == value, "fixed central protocol differs")
        d = copy.deepcopy(document)
        d["configurationId"] = "VARIANT"
        d["pretranscription"]["candidate"] = "VARIANT"
        if "variant" in d["measurement"]:
            check(d["measurement"]["variant"] == ("confirmed-pause-pretranscription" if candidate else
                                                  "matched-full-recording-reference"), "variant label differs")
            d["measurement"]["variant"] = "VARIANT"
        check(d["preprocessing"]["mode"] == (
            "continuous stable DSP; confirmed-pause prefixes and one final rest; per-job unchanged reference energy/VAD/ASR preparation"
            if candidate else "matched full-recording reference; unchanged energy/VAD/ASR preparation"),
            "preprocessing mode differs")
        d["preprocessing"]["mode"] = "VARIANT"
        normalized.append(d)
    check(normalized[0] == normalized[1], "parameter pair differs beyond the explicit variant")


def statistics(cases, values):
    check(len(values) == 180 and all(integer(x) for x in values), "all180 latency samples required")
    summaries = []
    for i,c in enumerate(cases):
        xs = sorted(values[i*30:(i+1)*30])
        median = (xs[14]+xs[15])/2
        p95 = xs[28]
        summaries.append(dict(fixtureId=c["id"], fixtureSha256=c["audio"]["sha256"],
            language=c["language"], durationSeconds=c["audio"]["frames"]//16000,
            statistics=dict(count=30, medianNanos=median, p95Nanos=p95, minNanos=xs[0],maxNanos=xs[-1],rangeNanos=xs[-1]-xs[0]),
            medianLimitPassed=median <= 1_000_000_000, p95LimitPassed=p95 <= 1_500_000_000))
    return dict(lNanos=sum(c["statistics"]["medianNanos"] for c in summaries)/6,
                absoluteLimitsPassed=all(c["medianLimitPassed"] and c["p95LimitPassed"] for c in summaries),cases=summaries)


def validate_row(row, result, slot, previous_jobs):
    check(row["id"] == slot["id"] and row["warmup"] is slot["warmup"] and row["status"] ==
          result["status"] == "ok" and row["error"] is None, "central row identity/status differs")
    check(row["session"] == (slot["fixtureIndex"]*30+slot["runIndex"]+1 if not slot["warmup"] else 1) and
          integer(row["session"],1) and row["terminal"] == row["expectedOutcome"] == "Local" and
          row["controllerClockMatchesCapture"] is True and row["referenceMatch"] is None, "central session/outcome differs")
    check(row["qosError"] is None, "publisher error")
    common.validate_publisher(row)
    frames = slot["durationSeconds"]*16000
    elapsed = row["engineElapsedNanos"]
    check(integer(elapsed) and type(row["engineInputFrames"]) is int and row["engineInputFrames"] == frames and
          row["engineRtfUnavailableReason"] is None and type(row["engineRtf"]) in (float,int) and math.isfinite(row["engineRtf"]) and
          math.isclose(row["engineRtf"], elapsed*16000/frames/1e9, rel_tol=1e-12, abs_tol=1e-15), "invalid engine timing")
    feed = row["feed"]; planned=frames*62500; pubs=feed["publications"]
    check(feed["timingValid"] is True and feed["plannedStopOffset"] == dict(secs=planned//1_000_000_000,nanos=0) and
          isinstance(pubs,list) and len(pubs) == frames//320, "feed count/stop differs")
    previous=0
    for i,p in enumerate(pubs):
        deadline=(i+1)*20_000_000
        before,after=p["beforePublishNanos"],p["afterPublishNanos"]
        check(p["startFrame"] == i*320 and p["endFrame"] == (i+1)*320 and p["plannedNanos"] == deadline and
              integer(before) and integer(after) and deadline <= before <= after <= deadline+10_000_000 and previous <= before and
              p["deviationNanos"] == after-deadline and p["timing"] == "onBudget" and "diagnostics" not in p,
              "publication gap/early/late/invalid evidence")
        previous=after
    check(feed["waitSummary"]["observedPublications"] == len(pubs) and feed["waitSummary"]["detailedPublications"] == 0, "wait evidence incomplete")
    stop,terminal=row["tStopOffsetNanos"],row["tTerminalOffsetNanos"]
    check(integer(stop) and planned <= stop <= planned+10_000_000 and stop >= previous and integer(terminal) and terminal >= stop and
          row["tTextOffsetNanos"] == terminal and row["stopToTextNanos"] == row["stopToTerminalNanos"] == terminal-stop and
          isinstance(result["text"],str) and 0 < len(result["text"].encode()) <= 256*1024 and bool(result["text"].strip()), "invalid final text/stop")
    screen.validate_trace(row["jobTrace"],row,frames,slot["language"])
    jobs=row["jobTrace"]["trace"]["jobs"]
    check(previous_jobs is None or jobs[0]["jobId"] > previous_jobs[-1]["jobId"], "job identity reused across sessions")
    return jobs


def validate_documents(plan, variant, results, meta, result_sha256):
    check(variant in IDS and isinstance(results,list) and isinstance(meta,dict), "malformed block")
    expected=slots(plan["cases"])
    check(len(results) == len(meta["runs"]) == 181, "all181 slots required")
    check(meta["schemaVersion"] == 1 and meta["kind"] == "paced-six-case-pretranscription-experiment" and
          meta["measurementComplete"] is True and meta["qualified"] is False and meta["p05Qualified"] is False and
          meta["qualityEvaluated"] is False and meta["debugAssertions"] is False and integer(meta["processId"],1), "metadata flags/PID invalid")
    check(meta["sourceRevision"] == plan["sourceRevision"] and meta["manifestSha256"] == plan["manifestSha256"] and
          meta["parameterDocumentSha256"] == plan["parameterHashes"][variant] and meta["parameters"] == plan["parameters"][variant] and
          meta["model"] == {k:plan["model"][k] for k in ("id","sha256")}, "execution binding differs")
    check(meta["warmupRuns"] == 1 and meta["measuredRunsPerCase"] == 30 and meta["requestedRows"] == 181 and
          meta["pretranscriptionCandidate"] is (variant == "candidate") and meta["contextCleanup"] is None and
          meta["initialFailure"] is None and meta["cleanup"]["joined"] is True and meta["cleanup"].get("error") is None,
          "mode/count/cleanup invalid")
    fixtures=[dict(id=c["id"],language=c["language"],**{"class":"clean","split":"acceptance"},
        durationSeconds=c["audio"]["frames"]//16000,audioSha256=c["audio"]["sha256"],inputFrames=c["audio"]["frames"],
        sampleRate=16000,referenceUtf8Sha256=common.sha256(c["reference"].encode())) for c in plan["cases"]]
    check(meta["fixtures"] == fixtures, "fixture/reference bindings differ")
    rows=[]; previous=None; latencies=[]
    for slot,result,record in zip(expected,results,meta["runs"]):
        check(result["slot"] == record["slot"] == slot, "foreign/duplicate/reordered slot")
        row=record["observation"]
        previous=validate_row(row,result,slot,previous)
        rows.append(dict(slot=slot,status="ok",validated=True,session=row["session"],
            stopToTextNanos=row["stopToTextNanos"],engineElapsedNanos=row["engineElapsedNanos"],engineRtf=row["engineRtf"]))
        if not slot["warmup"]: latencies.append(row["stopToTextNanos"])
    check(meta["jobTraceAfterCleanupError"] is None and meta["jobTraceAfterCleanup"] ==
          meta["runs"][-1]["observation"]["jobTrace"]["trace"], "postjoin trace changed")
    summary=statistics(plan["cases"],latencies)
    check(meta["statistics"] == summary, "reported central statistics differ")
    check(meta["hypothesesSha256"] == result_sha256, "hypothesis serialization binding differs")
    return dict(rows=rows,statistics=summary,processId=meta["processId"])


def immutable_sources(root, revision, sources, pins, fingerprints):
    bindings={b["path"]:b for b in sources}
    check(len(bindings) == len(sources) and (set(pins)|OWN).issubset(bindings), "missing/duplicate source bindings")
    for path,b in bindings.items():
        data=common.verify_binding(root,b,fingerprints,common.MAX_JSON,True)
        check(path not in pins or b["sha256"] == pins[path], "compiled source mismatch")
        obj=revision+":"+path
        size=subprocess.run(["git","cat-file","-s",obj],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10,check=False)
        check(size.returncode == 0 and size.stdout.strip() == str(len(data)).encode(), "immutable source size differs")
        committed=subprocess.run(["git","show",obj],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10,check=False)
        check(committed.returncode == 0 and common.sha256(committed.stdout) == b["sha256"], "immutable source differs")


def validate_quality_decision(root, binding, fingerprints, parameters):
    root=Path(root).resolve()
    decision=common.parse_json(common.verify_binding(root,binding,fingerprints,common.MAX_JSON,True))
    check(decision["kind"] == "pretranscription-quality-decision-v1" and decision["passed"] is True and
          decision["productionActivated"] is False, "bound quality approval required")
    common.verify_binding(root,decision["screeningFreeze"],fingerprints,common.MAX_JSON)
    quality=common.parse_json(common.verify_binding(root,decision["qualityReport"],fingerprints,common.MAX_METADATA,True))
    check(quality["schemaVersion"] == 1 and
          quality["kind"] == "pretranscription-quality-screening-decision-v1",
          "unknown quality report schema")
    check(quality["decision"]["numericalScreenPassed"] is True and
          quality["decision"]["candidateRejected"] is False,
          "quality report does not explicitly pass numerical screening")
    # Overall semantics/anchor acceptance remains the independent external
    # decision.passed authority; no invented semantic-report field is required.
    check(quality["decision"]["productionPretranscriptionEnabled"] is False and
          quality["decision"]["performanceRunStarted"] is False and
          quality["qualified"] is False and quality["L"] is None,
          "quality report has incompatible activation/performance claims")
    completion=common.parse_json(common.verify_binding(root,decision["screeningCompletion"],fingerprints,common.MAX_METADATA,True))
    check(completion["completed"] is True and len(completion["blocks"]) == 4, "quality screen not complete")
    for block in completion["blocks"]:
        check(len(block["cases"]) == 100,"quality screen block incomplete")
        for case in block["cases"]:
            check(case["status"] == "ok" and len(case["rows"]) == 2 and
                  all(r["status"] == "ok" and r["validated"] is True for r in case["rows"]),
                  "quality screen contains invalid/unavailable rows")
    check(decision["runtime"] == parameters["reference"]["runtime"] and
          decision["candidatePretranscription"] == parameters["candidate"]["pretranscription"],
          "quality decision does not cover these runtime/candidate parameters")
    return decision


def check_pair_targets(output, pair=None):
    check(pair is None or pair in PAIRS,"unknown pair role")
    for selected in ([pair] if pair is not None else PAIRS):
        check(not os.path.lexists(output/selected),"pair target already exists; no resume")


def preflight(path,root,pair=None):
    root=Path(root).resolve();raw=common.bounded_bytes(path,common.MAX_JSON); f=common.parse_json(raw); fp={}
    check(f["schemaVersion"] == 1 and f["kind"] == "prospective-pretranscription-central-pairs-v1", "freeze schema differs")
    check(re.fullmatch("[0-9a-f]{40}",f["sourceRevision"]) is not None, "full source revision required")
    check(f["binary"]["path"] == "target/release/examples/paced_reference_probe", "only central binary allowed")
    common.verify_binding(root,f["binary"],fp)
    parameters={v:common.parse_json(common.verify_binding(root,b,fp,1024*1024,True)) for v,b in f["parameters"].items()}
    validate_parameters(parameters)
    pins={}
    for d in parameters.values():
        for name,digest in common.compiled_sources(d).items():
            check(name not in pins or pins[name] == digest, "parameter source conflicts")
            pins[name]=digest
    immutable_sources(root,f["sourceRevision"],f["sources"],pins,fp)
    validate_quality_decision(root,f["qualityDecision"],fp,parameters)
    manifest=common.parse_json(common.verify_binding(root,f["manifest"],fp,common.MAX_JSON,True))
    cases=canonical_cases(manifest)
    inputs={b["path"]:b for b in f["inputs"]}
    check(len(inputs) == len(f["inputs"]) == 6 and set(inputs) == {c["audio"]["path"] for c in cases}, "six exact input bindings required")
    import wave
    for c in cases:
        b=inputs[c["audio"]["path"]];check(b["sha256"] == c["audio"]["sha256"],"input manifest hash differs")
        common.verify_binding(root,b,fp,20*1024*1024)
        with wave.open(str(common.relative_path(root,b["path"])),"rb") as wav:
            check((wav.getnchannels(),wav.getframerate(),wav.getsampwidth(),wav.getnframes(),wav.getcomptype()) ==
                  (1,16000,2,c["audio"]["frames"],"NONE"),"PCM header differs")
    model,vad=f["model"],f["vad"]
    check(model["id"] == "large-v3-turbo-q5_0" and model["sha256"] == common.MODEL_SHA and model["bytes"] == 574041195 and
          vad["sha256"] == common.VAD_SHA and vad["bytes"] == 885098,"model/VAD identity differs")
    check(Path(model["path"]).name == "ggml-large-v3-turbo-q5_0.bin" and Path(vad["path"]).name ==
          "ggml-silero-v5.1.2.bin" and Path(model["path"]).parent == Path(vad["path"]).parent,"model/VAD sibling required")
    for d in parameters.values():
        check(any(x["id"] == model["id"] and x["sha256"] == model["sha256"] for x in d["models"]) and
              d["runtime"]["speechGate"]["model"]["sha256"] == vad["sha256"],"parameter/model differs")
    common.verify_binding(root,model,fp);common.verify_binding(root,vad,fp)
    output=common.relative_path(root,f["outputDirectory"])
    check(Path(f["outputDirectory"]).parts[0] == "artifacts" and
          (not os.path.lexists(output) or output.is_dir()),"output root must be ignored artifacts")
    artifacts=(root/"artifacts").resolve()
    check(not output.is_symlink() and artifacts in output.resolve().parents,
          "output root must remain below actual artifacts directory")
    check_pair_targets(output,pair)
    return dict(root=root,cases=cases,sourceRevision=f["sourceRevision"],parameters=parameters,
        parameterHashes={v:b["sha256"] for v,b in f["parameters"].items()},
        parameterPaths={v:common.relative_path(root,b["path"]) for v,b in f["parameters"].items()},
        binary=common.relative_path(root,f["binary"]["path"]),manifest=common.relative_path(root,f["manifest"]["path"]),
        manifestSha256=f["manifest"]["sha256"],model=model,modelPath=common.relative_path(root,model["path"]),
        outputRoot=output,fingerprints=fp,freezeBytes=raw,freezeSha256=common.sha256(raw))


def paths_for(output,index,variant):
    result=output/("%d-%s.json"%(index+1,variant))
    return dict(results=result,metadata=Path(str(result)+".metadata.json"),log=Path(str(result)+".command.log"))


def command_for(plan,variant,paths):
    return ["/usr/bin/time","-l",str(plan["binary"]),str(plan["modelPath"]),"reference",str(plan["manifest"]),
            str(plan["parameterPaths"][variant]),str(paths["results"])]


def launch_block(plan,variant,paths):
    # Reuse the reviewed own-process-group/watchdog implementation verbatim.
    return screen.launch_case(plan,dict(id=variant,command=command_for(plan,variant,paths)),paths)


def execute_pair(plan,pair,launch=launch_block):
    check(pair in PAIRS,"unknown pair role")
    # Serializes only this freeze's owned pair processes. A stale lock fails closed.
    # Never remove a pre-existing lock or adopt/resume partial pair output.
    lock=plan["outputRoot"]/".active-pair"
    descriptor=os.open(lock,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    try:
        os.close(descriptor)
        return _execute_pair(plan,pair,launch)
    finally:
        os.unlink(lock)


def _execute_pair(plan,pair,launch):
    check(pair in PAIRS,"unknown pair role")
    output=plan["outputRoot"]/pair
    output.mkdir(mode=0o700);output.chmod(0o700)
    common.write_private_bytes(output/"freeze.json",plan["freezeBytes"])
    report=dict(schemaVersion=1,kind="pretranscription-central-pair-execution",pair=pair,
        freezeSha256=plan["freezeSha256"],startedUtc=screen.now(),completed=False,qualified=False,
        blocks=[],comparison=None,powerAssertionEnded=None)
    failure=None
    for index,variant in enumerate(PAIRS[pair]):
        entry=dict(variant=variant,status="unavailable",error=failure,
            rows=[dict(slot=s,status="unavailable",validated=False) for s in slots(plan["cases"])])
        if failure is None:
            paths=paths_for(output,index,variant)
            try:
                common.check_unchanged(plan)
                check(not screen._INTERRUPTED,"supervisor interrupted; no further block")
                launch_result=launch(plan,variant,paths)
                entry["launch"]=launch_result
                # Preserve raw slot statuses even when the process failed.
                if paths["results"].exists():
                    rs=common.read_json(paths["results"],common.MAX_TEXT)
                    if isinstance(rs,list):
                        for target,raw in zip(entry["rows"],rs[:181]):
                            if isinstance(raw,dict): target["status"]=raw.get("status","invalid")
                check(launch_result.get("exitCode") == 0 and launch_result.get("terminationConfirmed") is True and
                      launch_result.get("launchError") is None,"block process failed or unconfirmed")
                result_bytes=common.bounded_bytes(paths["results"],common.MAX_TEXT)
                rs=common.parse_json(result_bytes)
                meta=common.read_json(paths["metadata"],common.MAX_METADATA)
                for p in (paths["results"],paths["metadata"],paths["log"]):
                    check(stat.S_IMODE(os.stat(p,follow_symlinks=False).st_mode) == 0o600,"raw mode differs")
                checked=validate_documents(plan,variant,rs,meta,common.sha256(result_bytes))
                entry.update(checked,status="ok")
            except (ValueError,OSError,KeyError,TypeError,IndexError,AttributeError,OverflowError):
                failure="block launch/report/owner/timing/provenance validation failed"
                entry.update(status="failed",error=failure)
            entry["rawFiles"]=common.raw_bindings(paths)
            if len(entry["rawFiles"]) != 3 or any(
                    "error" in binding or not isinstance(binding.get("sha256"),str) or
                    re.fullmatch("[0-9a-f]{64}",binding["sha256"]) is None
                    for binding in entry["rawFiles"]):
                failure="raw archive binding missing or invalid"
                entry.update(status="failed",error=failure)
                entry.pop("statistics",None)
                for row in entry["rows"]:
                    row["validated"]=False
        report["blocks"].append(entry)
    report["completed"]=failure is None
    if report["completed"]:
        by_variant={b["variant"]:b for b in report["blocks"]}
        ref=by_variant["reference"]["statistics"]["lNanos"]; cand=by_variant["candidate"]["statistics"]["lNanos"]
        check(ref > 0,"reference L must be positive")
        report["comparison"]=dict(referenceLNanos=ref,candidateLNanos=cand,
            improvementFraction=(ref-cand)/ref,relativeImprovementPassed=cand <= ref*.9,
            candidateAbsoluteLimitsPassed=by_variant["candidate"]["statistics"]["absoluteLimitsPassed"])
    report["finishedUtc"]=screen.now()
    common.write_private(output/"completion.json",report)
    return report


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze",type=Path);parser.add_argument("--root",type=Path,default=Path.cwd())
    parser.add_argument("--pair",choices=tuple(PAIRS));parser.add_argument("--execute",action="store_true")
    args=parser.parse_args(argv)
    plan=preflight(args.freeze,args.root,args.pair)
    if not args.execute:
        print(json.dumps(dict(preflightPassed=True,pairs=list(PAIRS),rowsPerBlock=181)))
        return 0
    check(args.pair is not None,"explicit complete pair required")
    # Only a new pair is legal; the other completed pair may be from another window.
    plan["outputRoot"].mkdir(mode=0o700,exist_ok=True)
    power=screen.power_snapshot()
    check(power.get("source") == "AC Power" and power.get("error") is None,"AC Power required before launch")
    assertion=None
    try:
        assertion=subprocess.Popen(["/usr/bin/caffeinate","-i","-w",str(os.getpid())],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        check(assertion.poll() is None,"owned idle assertion failed")
        for sig in (signal.SIGINT,signal.SIGTERM): signal.signal(sig,screen.interrupt)
        def guarded(plan,variant,paths):
            check(assertion.poll() is None,"idle assertion ended before launch")
            snapshot=screen.power_snapshot()
            check(snapshot.get("source") == "AC Power" and snapshot.get("error") is None,"AC Power unavailable before block")
            result=launch_block(plan,variant,paths)
            result["powerBefore"]=snapshot; result["powerAfter"]=screen.power_snapshot()
            if result["powerAfter"].get("source") != "AC Power" or result["powerAfter"].get("error") is not None:
                result["launchError"]="AC Power unavailable at block end"
            return result
        report=execute_pair(plan,args.pair,guarded)
        ended=screen.stop_power(assertion);assertion=None
        common.write_private(plan["outputRoot"]/args.pair/"supervisor-completion.json",
            dict(freezeSha256=plan["freezeSha256"],pair=args.pair,powerAssertionEnded=ended,
                 pairCompletion=common.small_binding(plan["outputRoot"]/args.pair/"completion.json")))
        return 0 if report["completed"] and ended else 1
    finally:
        if assertion is not None: screen.stop_power(assertion)


if __name__=="__main__":
    try: raise SystemExit(main())
    except (ValueError,OSError,KeyError,TypeError,AttributeError,IndexError,OverflowError):
        print(json.dumps(dict(failed=True,error="central freeze or execution rejected; inspect private evidence")))
        raise SystemExit(2)
