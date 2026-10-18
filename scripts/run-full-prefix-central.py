#!/usr/bin/env python3
"""Two complete FullPrefix central pairs; default CLI checks bindings only."""
import copy
import importlib.util
from pathlib import Path


def sibling(name,label):
    spec=importlib.util.spec_from_file_location(label,Path(__file__).with_name(name))
    module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


central=sibling("run-pretranscription-central.py","full_prefix_shared_central")
full=sibling("run-full-prefix-screening.py","full_prefix_central_trace")
common,process=full.common,full.process
check,integer=common.check,common.integer
KIND="prospective-full-prefix-central-pairs-v1"
CENTRAL_MANIFEST_PATH="tests/fixtures/corpus/central-reference-v1.json"
CENTRAL_MANIFEST_SHA="244b2cd766874855174699f8568dbb09ae664380a98b272bf6588293f44a36b5"
IDS={v:"p06-six-case-full-prefix-"+v+"-v1" for v in ("reference","candidate")}
OWN=central.OWN|{"scripts/run-full-prefix-central.py","tests/quality/test_full_prefix_central.py",
    "docs/verification/full-prefix-central-freeze-schema.json","scripts/run-full-prefix-screening.py",
    "scripts/screening_quality/decision.py"}


def validate_parameters(parameters):
    check(set(parameters)==set(IDS),"both explicit full-prefix batch arms required")
    normalized=[]
    for variant,document in parameters.items():
        candidate=variant=="candidate"
        check(document["configurationId"]==IDS[variant],"wrong full-prefix batch configuration")
        checked=copy.deepcopy(document)
        checked["configurationId"]="p06-full-prefix-"+variant+"-v1"
        full.validate_parameters(checked,"FullPrefixCandidate" if candidate else "FullPrefixReference")
        for field,value in dict(warmupRuns=1,measuredRunsPerCase=30,totalRows=181).items():
            check(type(document["measurement"][field]) is int and document["measurement"][field]==value,
                  "one warmup/six-by-thirty protocol differs")
        check("measuredRuns" not in document["measurement"],"single-fixture run selector in batch")
        d=copy.deepcopy(document)
        d["configurationId"]="VARIANT"
        d["fullPrefix"]["candidate"]="VARIANT"
        check(d["measurement"]["variant"]==("full-prefix-revisions" if candidate else "matched-full-recording-reference"),
              "full-prefix variant label differs")
        d["measurement"]["variant"]="VARIANT"
        check(d["preprocessing"]["mode"]==(
            "continuous stable DSP; cumulative full-prefix revisions and complete final recording; current normal energy/VAD/preparation before every lookup"
            if candidate else "matched full recording only; same scoped cache decoder and normal energy/VAD/preparation; fresh scope cannot reuse prior session"),
            "full-prefix preprocessing label differs")
        d["preprocessing"]["mode"]="VARIANT"
        normalized.append(d)
    check(full.exact_json(normalized[0],normalized[1]),"batch arms differ beyond explicit variant")


def validate_row(row,result,slot,previous_jobs,candidate):
    check(row["id"]==slot["id"] and row["warmup"] is slot["warmup"] and row["status"]==result["status"]=="ok" and
          row["error"] is None,"central row identity/status differs")
    session=1 if slot["warmup"] else slot["fixtureIndex"]*30+slot["runIndex"]+1
    check(integer(row["session"],1) and row["session"]==session and
          row["terminal"]==row["expectedOutcome"]=="Local" and row["controllerClockMatchesCapture"] is True and
          row["referenceMatch"] is None and row["qosError"] is None,"central session/outcome differs")
    common.validate_publisher(row)
    frames=slot["durationSeconds"]*16000
    process.validate_feed(row,frames)
    stop,terminal=row["tStopOffsetNanos"],row["tTerminalOffsetNanos"]
    check(integer(terminal) and terminal>=stop and row["tTextOffsetNanos"]==terminal and
          row["stopToTextNanos"]==row["stopToTerminalNanos"]==terminal-stop and
          isinstance(result["text"],str) and bool(result["text"].strip()) and
          len(result["text"].encode())<=256*1024 and row["engineRtfUnavailableReason"] is None,
          "final text or terminal timing differs")
    summary=full.validate_trace(row["jobTrace"],row,frames,slot["language"],candidate)
    jobs=row["jobTrace"]["trace"]["jobs"]
    check(previous_jobs is None or jobs[0]["jobId"]>previous_jobs[-1]["jobId"],"job IDs reused across sessions")
    return jobs,summary


def validate_documents(plan,variant,results,meta,result_sha256):
    check(variant in IDS and isinstance(results,list) and isinstance(meta,dict),"malformed full-prefix block")
    host=meta.get("hostObservation")
    power=host.get("powerSource") if isinstance(host,dict) else None
    check(isinstance(power,dict) and power.get("value")=="AC Power" and
          "error" in power and power["error"] is None,"actual probe AC Power evidence required")
    expected=central.slots(plan["cases"])
    check(len(results)==len(meta["runs"])==181,"all181 central slots required")
    check(type(meta["schemaVersion"]) is int and meta["schemaVersion"]==1 and
          meta["kind"]=="paced-six-case-full-prefix-experiment" and meta["measurementComplete"] is True and
          meta["qualified"] is False and meta["p05Qualified"] is False and meta["qualityEvaluated"] is False and
          meta["debugAssertions"] is False and integer(meta["processId"],1),"full-prefix metadata flags/PID differ")
    check(meta["sourceRevision"]==plan["sourceRevision"] and meta["manifestSha256"]==plan["manifestSha256"] and
          meta["parameterDocumentSha256"]==plan["parameterHashes"][variant] and
          full.exact_json(meta["parameters"],plan["parameters"][variant]) and
          full.exact_json(meta["model"],{k:plan["model"][k] for k in ("id","sha256")}),"execution bindings differ")
    check(all(type(meta[field]) is int and meta[field]==value for field,value in
              dict(warmupRuns=1,measuredRunsPerCase=30,requestedRows=181).items()) and
          meta["fullPrefixCandidate"] is (variant=="candidate") and meta["pretranscriptionCandidate"] is None and
          meta["contextCleanup"] is None and meta["initialFailure"] is None and meta["cleanup"]["joined"] is True and
          meta["cleanup"]["error"] is None,"mode/count/owner cleanup differs")
    fixtures=[dict(id=c["id"],language=c["language"],**{"class":"clean","split":"acceptance"},
        durationSeconds=c["audio"]["frames"]//16000,audioSha256=c["audio"]["sha256"],inputFrames=c["audio"]["frames"],
        sampleRate=16000,referenceUtf8Sha256=common.sha256(c["reference"].encode())) for c in plan["cases"]]
    check(full.exact_json(meta["fixtures"],fixtures),"fixture/reference metadata differs")
    rows,latencies,previous=[],[],None
    for slot,result,record in zip(expected,results,meta["runs"]):
        check(set(result)=={"slot","status","text"} and full.exact_json(result["slot"],slot) and
              full.exact_json(record["slot"],slot),"foreign/duplicate/reordered slot")
        row=record["observation"]
        previous,summary=validate_row(row,result,slot,previous,variant=="candidate")
        rows.append(dict(slot=slot,status="ok",validated=True,session=row["session"],
            stopToTextNanos=row["stopToTextNanos"],**summary))
        if not slot["warmup"]:latencies.append(row["stopToTextNanos"])
    check(meta["jobTraceAfterCleanupError"] is None and
          full.exact_json(meta["jobTraceAfterCleanup"],meta["runs"][-1]["observation"]["jobTrace"]["trace"]),
          "postjoin trace differs")
    summary=central.statistics(plan["cases"],latencies)
    check(full.exact_json(meta["statistics"],summary),"reported batch statistics differ")
    check(meta["hypothesesSha256"]==result_sha256,"hypotheses byte binding differs")
    return dict(rows=rows,statistics=summary,processId=meta["processId"])


def validate_quality(root,binding,fingerprints,parameters):
    spec=importlib.util.spec_from_file_location("full_prefix_central_decision",Path(root)/"scripts/screening_quality/decision.py")
    module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.validate(root,binding,fingerprints,parameters)


def execute_pair(plan,pair,launch=None):
    return central.execute_pair(plan,pair,central.launch_block if launch is None else launch,
        validator=validate_documents,kind="full-prefix-central-pair-execution-v1")


def preflight(path,root,pair=None):
    import os
    import stat
    import subprocess
    root=Path(root).resolve(strict=True)
    plan=central.preflight(path,root,pair,freeze_kind=KIND,parameters_validator=validate_parameters,
        quality_validator=validate_quality,required_sources=OWN)
    # Bind the exact bytes already parsed by shared preflight, not a later
    # replacement baseline. Every pair then checks this same fingerprint plan.
    frozen_path=Path(path).resolve(strict=True)
    check(root in frozen_path.parents,"freeze outside repository")
    common.verify_binding(root,dict(path=str(frozen_path.relative_to(root)),bytes=len(plan["freezeBytes"]),
        sha256=plan["freezeSha256"],mode=oct(stat.S_IMODE(frozen_path.lstat().st_mode))),
        plan["fingerprints"],common.MAX_JSON)
    frozen=common.parse_json(plan["freezeBytes"])
    check(frozen["manifest"]["path"]==CENTRAL_MANIFEST_PATH and
          frozen["manifest"]["sha256"]==plan["manifestSha256"]==CENTRAL_MANIFEST_SHA,
          "fixed original central-six authority differs from remainder selection")
    check(type(frozen["schemaVersion"]) is int and frozen["schemaVersion"]==1 and
          frozen["qualified"] is False and frozen["L"] is None,"unqualified central freeze required")
    bindings={b["path"]:b for b in frozen["sources"]}
    for module,source in ((None,"scripts/run-full-prefix-central.py"),(central,"scripts/run-pretranscription-central.py"),
        (full,"scripts/run-full-prefix-screening.py"),(process,"scripts/run-pretranscription-screening.py"),
        (common,"scripts/run-paced-remainder.py")):
        actual=common.bounded_bytes(Path(__file__ if module is None else module.__file__),common.MAX_JSON)
        check(len(actual)==bindings[source]["bytes"] and common.sha256(actual)==bindings[source]["sha256"],
              "executing central validator differs from frozen source")
    check(os.access(plan["binary"],os.X_OK),"central binary not executable")
    for variant,document in plan["parameters"].items():
        flag="--print-full-prefix-"+variant+"-parameters"
        printed=subprocess.run([str(plan["binary"]),flag],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
            timeout=10,check=False)
        check(printed.returncode==0 and len(printed.stdout)<=1024*1024 and
              full.exact_json(common.parse_json(printed.stdout),document),"exact compiled full-prefix batch template differs")
    return plan


def main(argv=None):
    return central.main(argv,preflight_fn=preflight,executor_fn=execute_pair)


if __name__=="__main__":
    import json
    import subprocess
    import sys
    try:raise SystemExit(main())
    except (ValueError,OSError,KeyError,TypeError,AttributeError,IndexError,OverflowError,subprocess.SubprocessError):
        print(json.dumps(dict(failed=True,error="full-prefix central freeze/execution rejected; private evidence retained")))
        raise SystemExit(2)
