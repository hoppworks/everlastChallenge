"""One fresh FullPrefix freeze -> quality authorities; no continuation adoption.

The supplied protocol is the source-pinned reviewed full-prefix screener. Its
pure report validators are reused; no model, probe, evaluator or printer starts.
Model/input/binary descriptors retain their prospective binding, not a claim
that assembly rehashes or rebuilds those artifacts after the completed run.
"""
import copy
import hashlib
import re
from pathlib import Path
from .assembly import assemble, bind_private, canonical, check, reader
from .continuation_adapter import recorded_root, same_binding
from .profiles import FULL_PREFIX


def _relative(name, root):
    path=Path(name)
    if path.is_absolute():
        path=path.relative_to(root)
    check(not path.is_absolute() and bool(path.parts) and ".." not in path.parts,"unsafe bound case path")
    return str(path)


def _protocol_sources(root, freeze, protocol, load):
    check(protocol.KIND == "prospective-full-prefix-screening-v1" and tuple(protocol.VARIANTS) == FULL_PREFIX.variants,
          "wrong full-prefix validation protocol")
    sources={binding["path"]:binding for binding in freeze["sources"]}
    check(len(sources)==len(freeze["sources"]),"duplicate source authority")
    # Check the actual executing modules AND their bound repository copies.
    # There is no unpinned switch to the old disjoint validator.
    for module,path in ((protocol,"scripts/run-full-prefix-screening.py"),
                        (protocol.process,"scripts/run-pretranscription-screening.py"),
                        (protocol.common,"scripts/run-paced-remainder.py")):
        binding=sources[path]
        load(binding,"source")
        data=protocol.common.bounded_bytes(Path(module.__file__),16*1024*1024)
        check(len(data)==binding["bytes"] and hashlib.sha256(data).hexdigest()==binding["sha256"],
              "executing validator differs from frozen source")
    return [sources[path] for path in ("scripts/run-full-prefix-screening.py",
            "scripts/run-pretranscription-screening.py","scripts/run-paced-remainder.py")]


def prepare(root, freeze_binding, completion_binding, protocol):
    """Resolve all400 frozen case authorities before publishing any of800 rows.

    The freeze/global bindings must themselves be independently supplied. Case
    paths come from that freeze, never from completion origins. Every original
    case body and all three raw files are checked, including failed/missing-row
    rejection, before the shared byte-preserving assembly is called.
    """
    root=Path(root).resolve(strict=True)
    load=reader(root)
    freeze=load(freeze_binding,"authority")
    check(type(freeze.get("schemaVersion")) is int and freeze["schemaVersion"]==1 and
          freeze.get("kind")=="prospective-full-prefix-screening-v1","wrong full-prefix freeze")
    protocol_sources=_protocol_sources(root,freeze,protocol,load)
    check(recorded_root(freeze).resolve(strict=True)==root,"frozen command root differs")
    output=Path(freeze["outputDirectory"])
    check(not output.is_absolute() and ".." not in output.parts and output.parts[0]=="artifacts",
          "frozen output namespace differs")
    check(_relative(completion_binding["path"],root)==str(output/"completion.json"),"global completion path not frozen")
    completion=load(completion_binding,"completion")
    FULL_PREFIX.completion(completion)
    check(completion["freezeSha256"]==freeze_binding["sha256"] and
          completion["screeningId"]==freeze["screeningId"] and completion["sourceRevision"]==freeze["sourceRevision"] and
          re.fullmatch("[0-9a-f]{40}",freeze["sourceRevision"]) and freeze["qualified"] is False and freeze["L"] is None,
          "global source/freeze identity differs")
    check(freeze["manifest"]["sha256"]==protocol.process.MANIFEST_SHA and
          freeze["selection"]["sha256"]==protocol.process.SELECTION_SHA,"fixed100 authority changed")
    manifest=load(freeze["manifest"],"authority")
    selection=load(freeze["selection"],"authority")
    cases=manifest["cases"]
    check(manifest["unicodeVersion"]=="13.0.0" and len(cases)==len(selection["cases"])==100 and
          len({case["id"] for case in cases})==100,"full100 membership/Unicode differs")
    check(set(freeze["parameters"])==set(FULL_PREFIX.variants),"full-prefix parameter arms differ")
    parameters={variant:load(binding,"authority") for variant,binding in freeze["parameters"].items()}
    for variant,document in parameters.items():
        protocol.validate_parameters(document,variant)
    check(canonical(parameters[FULL_PREFIX.variants[0]]["runtime"])==canonical(parameters[FULL_PREFIX.variants[1]]["runtime"]),
          "full reference decoder policies differ")
    pins={}
    for document in parameters.values():
        for path,digest in protocol.common.compiled_sources(document).items():
            check(path not in pins or pins[path]==digest,"conflicting compiled source descriptors")
            pins[path]=digest
    sources={binding["path"]:binding for binding in freeze["sources"]}
    check((set(pins)|protocol.OWN_SOURCES).issubset(sources) and
          all(sources[path]["sha256"]==digest for path,digest in pins.items()),
          "compiled parameter/source descriptors differ")
    models={binding["path"]:binding for binding in freeze["models"]}
    check(len(models)==len(freeze["models"])==2 and set(models)=={protocol.process.MODEL_PATH,protocol.process.VAD_PATH} and
          models[protocol.process.MODEL_PATH]["sha256"]==protocol.common.MODEL_SHA and
          models[protocol.process.VAD_PATH]["sha256"]==protocol.common.VAD_SHA,"frozen model/VAD descriptors differ")
    model=dict(id="large-v3-turbo-q5_0",sha256=models[protocol.process.MODEL_PATH]["sha256"])
    check(freeze["binary"]["path"]=="target/release/examples/paced_runtime_probe" and
          tuple(block["variant"] for block in freeze["blocks"])==FULL_PREFIX.variants and
          [(b["name"],b["variant"],len(b["cases"])) for b in completion["blocks"]]==
          [(b["name"],b["variant"],100) for b in freeze["blocks"]] and len(freeze["blocks"])==4,
          "frozen four-block order/count differs")
    plan=dict(root=root,parameters=parameters,parameterBindings=freeze["parameters"],sourceRevision=freeze["sourceRevision"],model=model)
    authorities,seen_paths,seen_pids=[],set(),set()
    reserved={str(output/name) for name in ("freeze.json","launch.json","completion.json")}
    for bi,(block,done) in enumerate(zip(freeze["blocks"],completion["blocks"])):
        check(len(block["cases"])==100,"frozen block incomplete")
        for ci,(frozen_case,case,selected,entry) in enumerate(zip(block["cases"],cases,selection["cases"],done["cases"])):
            slot=bi*100+ci+1
            check(frozen_case["id"]==case["id"]==selected["id"]==entry["id"] and
                  all(case[key]==selected[key] for key in ("language","class","split","audio")) and
                  hashlib.sha256(case["reference"].encode()).hexdigest()==selected["referenceUtf8Sha256"],
                  "manifest/selection/frozen case identity differs")
            expected="Empty" if not case["reference"].strip() else "Local"
            check(selected["expectedOutcome"]==expected,"fixed expected outcome differs")
            for key in ("output","metadata","log","completion"):
                name=_relative(frozen_case[key],root)
                check(Path(name).parent==output and name not in seen_paths and name not in reserved,"reused or nonfrozen case path")
                seen_paths.add(name)
            check(frozen_case["metadata"]==frozen_case["output"]+".metadata.json","metadata target differs")
            command=["/usr/bin/time","-l",str(root/freeze["binary"]["path"]),str(root/protocol.process.MODEL_PATH),"reference",
                     str(root/case["audio"]["path"]),case["language"],str(root/freeze["parameters"][block["variant"]]["path"]),
                     str(root/frozen_case["output"]),"1"]
            if expected=="Empty":command.append("--expect-empty")
            check(frozen_case["command"]==command,"frozen one-warmup/one-measured command differs")
            binding=bind_private(root,frozen_case["completion"])
            original=load(binding,"completion")
            check(canonical(original)==canonical(entry),"original case/global completion differs")
            check(entry["status"]=="ok" and entry["error"] is None and entry["globalSlot"]==slot and
                  entry["screeningId"]==freeze["screeningId"] and entry["freezeSha256"]==freeze_binding["sha256"] and
                  entry["variant"]==block["variant"] and entry["blockName"]==block["name"],"failed/reordered full-prefix case")
            launch=entry["launch"]
            check(launch["command"]==command and type(launch["exitCode"]) is int and launch["exitCode"]==0 and launch["terminationConfirmed"] is True and
                  launch["launchError"] is None and launch["watchdogExpired"] is False and
                  type(launch["wrapperPid"]) is int and launch["wrapperPid"]>0 and
                  all(entry[key]["source"]=="AC Power" and entry[key]["error"] is None for key in ("beforePower","afterPower")),
                  "case launch/owner/power authority failed")
            raw=entry["rawFiles"]
            check(len(raw)==3,"three raw roles required")
            for actual,key,role in zip(raw,("output","metadata","log"),("results","metadata","log")):
                expected_raw=dict(actual,path=frozen_case[key])
                same_binding(actual,expected_raw,root)
                load(actual,role) # bind the bytes subsequently read by the protocol and assembly
            record=dict(selected,**{key:value for key,value in frozen_case.items() if key!="id"})
            paths={key:protocol.common.relative_path(root,frozen_case[key]) for key in ("output","metadata","log","completion")}
            validated=protocol.validate_case(plan,block,record,paths)
            check(validated["errors"]==[] and canonical(validated["rows"])==canonical(entry["rows"]) and
                  validated["processId"]==entry["actualProbePid"] and type(entry["actualProbePid"]) is int and
                  entry["actualProbePid"]>0 and entry["actualProbePid"] not in seen_pids,"raw pipeline validation differs")
            seen_pids.add(entry["actualProbePid"])
            metadata=dict(sourceRevision=freeze["sourceRevision"],processId=entry["actualProbePid"],
                parameterDocumentSha256=freeze["parameters"][block["variant"]]["sha256"],parameters=parameters[block["variant"]],
                model=model,fixtureSha256=case["audio"]["sha256"],inputFrames=case["audio"]["frames"],sampleRate=16000,channels=1,
                language=case["language"],expectedOutcome=expected,fullPrefixCandidate=block["variant"]=="FullPrefixCandidate",
                pretranscriptionCandidate=None,warmupRuns=1,measuredRunsRequested=1)
            authorities.append(dict(block=block["name"],variant=block["variant"],id=case["id"],completion=binding,
                rawFiles=copy.deepcopy(raw),metadata=metadata,origin=dict(freezeSha256=freeze_binding["sha256"],
                screeningId=freeze["screeningId"],sourceRevision=freeze["sourceRevision"],completionPath=binding["path"])))
    results,provenance=assemble(manifest,completion,authorities,load,profile=FULL_PREFIX)
    provenance["authorityBindings"]=dict(freeze=freeze_binding,completion=completion_binding,
        manifest=freeze["manifest"],selection=freeze["selection"],parameters=freeze["parameters"],validationProtocol=protocol_sources)
    provenance["assemblySourceBindings"]=[]
    for path in Path(__file__).parent.glob("*.py"):
        data=protocol.common.bounded_bytes(path,16*1024*1024)
        provenance["assemblySourceBindings"].append(dict(path=str(path),bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
    return manifest,results,provenance
