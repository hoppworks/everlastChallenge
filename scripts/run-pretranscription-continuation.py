#!/usr/bin/env python3
"""Explicit two-epoch continuation of the failed v2 screen, never v2 resume."""
import importlib.util
import copy
import json
import os
import stat
import subprocess
import uuid
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("screening",Path(__file__).with_name("run-pretranscription-screening.py"))
screen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(screen)
common = screen.common


OLD_FREEZE_PATH = "tests/fixtures/corpus/p06-pretranscription-screening-run-v2.json"
OLD_ARCHIVE_PATH = "docs/verification/p06-pretranscription-screening-v2-failed.json"
OLD_FREEZE_SHA = "96e11df8d0f33c0467f184712b4ad21ed0379ac684a0d5242f4f7fe52c580a81"
OLD_ARCHIVE_SHA = "6ed3f21b6569e37cafb607e4659911f6b511a4c5b7e04652226d64a5b031e48e"
OLD_REVISION = "4fa4d936cd35ffcc6055c1c1da9b25bf62ec9a72"
PREFIX = 185
TOTAL = 400
EXTRA_SOURCES = ("scripts/run-pretranscription-continuation.py",
                 "tests/quality/test_pretranscription_continuation.py",
                 "docs/verification/p06-pretranscription-continuation-v1.md")
check = common.check


def slots(freeze):
    check(len(freeze["blocks"])==4 and tuple(b["variant"] for b in freeze["blocks"])==screen.VARIANTS,
          "fixed four-block ABBA required")
    check(all(len(b["cases"])==100 for b in freeze["blocks"]),"four complete100-case blocks required")
    return [(b,i+1,c) for b in freeze["blocks"] for i,c in enumerate(b["cases"])]


def build_ledger(old_freeze, archive):
    planned=slots(old_freeze)
    check(old_freeze["sourceRevision"]==archive["sourceRevision"]==OLD_REVISION,"wrong original epoch")
    check(len(archive["cases"])==TOTAL,"missing original slots")
    check(archive["execution"]["caseStatuses"]=={"ok":185,"failed":1,"unavailable":214},"wrong fixed failure boundary")
    completions={b["path"]:b for b in archive["bindings"]["caseCompletions"]}
    raw={b["path"]:b for b in archive["bindings"]["rawFiles"]}
    check(len(completions)==400 and len(raw)==558,"incomplete original artifact inventory")
    entries=[]
    for ordinal,((block,index,expected),observed) in enumerate(zip(planned,archive["cases"]),1):
        check((observed["block"],observed["variant"],observed["caseIndex"],observed["id"])==
              (block["name"],block["variant"],index,expected["id"]),"original slot identity/order differs")
        status="ok" if ordinal<=PREFIX else "failed" if ordinal==PREFIX+1 else "unavailable"
        check(observed["status"]==status,"adoption must be exact accepted whole prefix185")
        check(len(observed["rows"])==2,"missing whole-case phase")
        if ordinal>PREFIX:
            continue
        check(observed["error"] is None and observed["beforePower"]["source"]==observed["afterPower"]["source"]=="AC Power"
              and observed["beforePower"]["error"] is None and observed["afterPower"]["error"] is None,"old case is not AC accepted")
        check(observed["cleanup"]["joined"] is True and observed["cleanup"]["error"] is None and
              observed["launch"]["exitCode"]==0 and observed["launch"]["terminationConfirmed"] is True,
              "old owner/process cleanup unverified")
        for phase,row in enumerate(observed["rows"]):
            check(row["runId"]==("warmup" if phase==0 else "measured-001") and row["warmup"] is (phase==0) and
                  row["status"]=="ok" and row["validated"] is True,"cannot adopt partial/invalid phase")
        check(expected["completion"] in completions and all(expected[k] in raw for k in ("output","metadata","log")),"missing old artifact binding")
        entries.append(dict(slot=ordinal,block=block["name"],caseIndex=index,id=expected["id"],
            originalSourceRevision=OLD_REVISION,completion=completions[expected["completion"]],
            rawFiles=[raw[expected[k]] for k in ("output","metadata","log")],
            processIdentity=dict(epoch=OLD_FREEZE_SHA,wrapperPid=observed["launch"]["wrapperPid"],
                                 probePid=observed["actualProbePid"],startedUtc=observed["launch"]["startedUtc"])))
    check(planned[PREFIX][0]["variant"]=="PretransCandidate" and planned[PREFIX][1]==86 and
          planned[PREFIX][2]["id"]=="constructed-de-masked-300s","failed186 must be repeated whole")
    return dict(schemaVersion=1,kind="fixed-v2-whole-case-adoption-ledger",originalFreezeSha256=OLD_FREEZE_SHA,
        originalArchiveSha256=OLD_ARCHIVE_SHA,originalSourceRevision=OLD_REVISION,entries=entries,
        adoptedCases=185,adoptedRows=370,newCases=215,newRows=430,rerunFromSlot=186,
        v3AdoptionAllowed=False,qualified=False,L=None)


def common_assets(old, new):
    for key in ("binary","parameters","models","manifest","selection","inputs"):
        check(old[key]==new[key],"shared runtime/model/input binding differs")
    old_slots,new_slots=slots(old),slots(new)
    for (ob,oi,oc),(nb,ni,nc) in zip(old_slots,new_slots):
        check((ob["name"],ob["variant"],oi,oc["id"])==(nb["name"],nb["variant"],ni,nc["id"]),
              "logical ABBA slot mapping changed")
        check(oc["command"][:8]==nc["command"][:8] and oc["command"][9:]==nc["command"][9:],
              "command changed outside output argument8")


def execute(plan, launch, finish=None):
    check(len(plan["adoption"])==PREFIX,"all185 whole inherited cases required")
    if "excludedV3" in plan:
        verify_excluded_epoch(plan["root"],plan["excludedV3"],plan["fingerprints"])
    startup=screen.power_snapshot();screen.require_ac(startup)
    output=plan["output"];output.mkdir(mode=0o700);output.chmod(0o700)
    for name,key in (("freeze.json","wrapperBytes"),("epoch-freeze.json","epochBytes"),("adoption-ledger.json","ledgerBytes")):
        if key in plan:
            common.write_private_bytes(output/name,plan[key])
    common.write_private(output/"launch.json",dict(plan.get("launchObservation",{}),startupPower=startup,
        continuationId=plan["continuationId"],freezeSha256=plan.get("freezeSha256"),
        wrapperSha256=plan.get("wrapperSha256"),adoptionLedgerSha256=plan.get("adoptionLedgerSha256"),
        adoptedCases=185,newCases=215,qualified=False))
    report=dict(schemaVersion=1,kind="pretranscription-continuation-execution",continuationId=plan["continuationId"],
                oldFreezeSha256=plan["oldFreezeSha256"],freezeSha256=plan.get("freezeSha256"),
                wrapperSha256=plan.get("wrapperSha256"),adoptionLedgerSha256=plan.get("adoptionLedgerSha256"),
                blocks=[],completed=False,qualified=False,L=None,
                adoptedCases=185,adoptedRows=370,newCasesRequested=215,newRowsRequested=430,plannedRows=800,error=None)
    failure=None;ordinal=0;new_starts=0;seen=set()
    for block in plan["blocks"]:
        projected=dict(name=block["name"],variant=block["variant"],cases=[])
        for case in block["cases"]:
            ordinal+=1
            paths=screen.paths_for(plan,case)
            if ordinal<=PREFIX:
                entry=copy.deepcopy(plan["adoption"][ordinal-1])
                check(entry["id"]==case["id"] and entry["status"]=="ok","inherited case order changed")
                entry.update(adopted=True,origin=dict(epoch=plan["oldFreezeSha256"],sourceRevision=plan["oldSourceRevision"],
                    completionPath=plan["adoptionLedger"]["entries"][ordinal-1]["completion"]["path"]))
            else:
                entry=dict(id=case["id"],status="unavailable",error=failure,rows=screen.canonical_rows(case,"unavailable"),
                           adopted=False,origin=dict(epoch=plan["continuationId"],sourceRevision=plan["sourceRevision"],
                                                     completionPath=case["completion"]))
                if failure is None and screen._INTERRUPTED:
                    failure="continuation interrupted; no later cases started"
                if failure is None:
                    before=screen.power_snapshot();after=None
                    try:
                        screen.require_ac(before);common.check_unchanged(plan)
                        launched=launch(case,paths);new_starts+=1
                        after=screen.power_snapshot()
                    except (ValueError,OSError,KeyboardInterrupt) as error:
                        launched=dict(exitCode=None,terminationConfirmed=False,launchError="prelaunch failed: "+type(error).__name__)
                    errors=[]
                    if after is not None and (after.get("source")!="AC Power" or after.get("error") is not None):
                        errors.append("post-probe AC Power evidence missing or battery observed")
                    if launched.get("exitCode")!=0:errors.append("wrapped probe failed")
                    if launched.get("terminationConfirmed") is not True:errors.append("owned process end unconfirmed")
                    if launched.get("launchError"):errors.append(launched["launchError"])
                    validation=screen.validate_case(plan,block,case,paths)
                    errors.extend(validation["errors"])
                    raw=common.raw_bindings(dict(results=paths["output"],metadata=paths["metadata"],log=paths["log"]))
                    if any(r.get("error") or r.get("sha256") is None for r in raw):errors.append("raw binding unavailable")
                    pid=validation.get("processId")
                    identity=(plan["continuationId"],pid,launched.get("startedUtc"))
                    if pid is not None and identity in seen:errors.append("duplicate process identity in new epoch")
                    if pid is not None:seen.add(identity)
                    failure="; ".join(errors) if errors else None
                    entry.update(status="failed" if failure else "ok",error=failure,rows=validation["rows"],
                        launch=launched,actualProbePid=pid,rawFiles=raw,beforePower=before,afterPower=after)
                elif entry["error"] is None:
                    entry["error"]=failure
            common.write_private(paths["completion"],entry)
            projected["cases"].append(entry)
        report["blocks"].append(projected)
    check(ordinal==TOTAL,"missing full800 mapping")
    if finish is not None:
        report["powerAssertionEnded"]=finish()
        if report["powerAssertionEnded"] is not True:failure=failure or "idle-sleep assertion cleanup unconfirmed"
    report.update(completed=failure is None,error=failure,newLaunchAttempts=new_starts,rowCount=800)
    common.write_private(output/"completion.json",report)
    return report


def bind(root, path):
    name=common.relative_path(root,path)
    data=common.bounded_bytes(name,1024*1024*1024)
    return dict(path=path,bytes=len(data),sha256=common.sha256(data),
                mode=oct(stat.S_IMODE(name.stat().st_mode)))


def prepared_bind(root, path, value):
    target=common.relative_path(root,path)
    common.write_private(target,value)
    return bind(root,path)


def prepare(root, template_path, revision, continuation_id, directory, epoch_output, ledger_output, output, owner_clearance=None):
    root=Path(root).resolve(strict=True)
    check(str(uuid.UUID(continuation_id))==continuation_id,"canonical fresh UUID required")
    check(subprocess.check_output(["git","rev-parse","HEAD"],cwd=root,text=True).strip()==revision and
          len(revision)==40,"explicit final committed HEAD required")
    for name in (epoch_output,ledger_output,output):
        check(not os.path.lexists(common.relative_path(root,name)),"new freeze/ledger target already exists")
    run_dir=common.relative_path(root,directory)
    check(Path(directory).parts[0]=="artifacts" and not os.path.lexists(run_dir) and run_dir.parent.is_dir(),
          "new run namespace must remain absent")
    old_raw=common.bounded_bytes(root/OLD_FREEZE_PATH,common.MAX_JSON)
    archive_raw=common.bounded_bytes(root/OLD_ARCHIVE_PATH,common.MAX_JSON)
    check(common.sha256(old_raw)==OLD_FREEZE_SHA and common.sha256(archive_raw)==OLD_ARCHIVE_SHA,"original pinned history differs")
    old=common.parse_json(old_raw);archive=common.parse_json(archive_raw)
    template=common.read_json(common.relative_path(root,template_path),common.MAX_JSON)
    check(template["runId"]=="p06-pretranscription-screening-v3","v3 template/exclusion source required")
    excluded=dict(freeze=bind(root,template_path),
                  launch=bind(root,template["outputDirectory"]+"/launch.json"),
                  completion=bind(root,template["outputDirectory"]+"/completion.json"))
    if owner_clearance is not None:
        excluded["ownerClearance"]=bind(root,owner_clearance)
    verify_excluded_epoch(root,excluded,{})
    common_assets(old,template)
    ledger=build_ledger(old,archive)
    epoch=copy.deepcopy(template);epoch["sourceRevision"]=revision
    epoch["runId"]="pretranscription-continuation-"+continuation_id
    epoch["outputDirectory"]=directory
    # Rebind legitimate current documentation additions; compiled runtime below
    # remains exactly the same. Historical sources are verified separately at4fa.
    sources={b["path"]:b for b in template["sources"]}
    for name in EXTRA_SOURCES:sources[name]=None
    check(len(sources)==152,"149 existing plus three continuation sources required")
    for name in sources:
        actual=bind(root,name)
        data=subprocess.check_output(["git","show",revision+":"+name],cwd=root)
        check(common.sha256(data)==actual["sha256"],"source not equal to final immutable commit")
        sources[name]=actual
    epoch["sources"]=[sources[name] for name in sorted(sources)]
    for b in epoch["blocks"]:
        for c in b["cases"]:
            for key in ("output","metadata","log","completion"):
                c[key]=str(Path(directory)/Path(c[key]).name)
            c["command"][8]=str(root/c["output"])
    selected=common.read_json(root/epoch["selection"]["path"],common.MAX_JSON)["cases"]
    screen.validate_blocks(epoch,root,selected)
    common_assets(old,epoch)
    # Verify the unchanged35 compiled pins without initializing a model.
    compiled={}
    for parameter in epoch["parameters"].values():
        raw=common.bounded_bytes(root/parameter["path"],1024*1024)
        check(common.sha256(raw)==parameter["sha256"],"parameter file changed")
        compiled.update(common.compiled_sources(common.parse_json(raw)))
    check(len(compiled)==35 and all(sources[k]["sha256"]==v for k,v in compiled.items()),"compiled runtime changed")
    provenance=epoch["build"]["rustSourcesUnchangedFrom"]
    result=subprocess.run(["git","diff","--quiet",provenance,revision,"--",":(glob)**/*.rs",":(glob)**/Cargo.toml","Cargo.toml","Cargo.lock"],cwd=root,check=False)
    check(result.returncode==0,"binary Rust/Cargo provenance changed")
    # Heavy model/input hashing is deliberately performed once by run preflight,
    # not by this prospective generator while an independent v3 run is active.
    ledger["continuationId"]=continuation_id
    wrapper=dict(schemaVersion=1,kind="prospective-pretranscription-continuation-v1",continuationId=continuation_id,
        sourceRevision=revision,originalFreeze=bind(root,OLD_FREEZE_PATH),originalArchive=bind(root,OLD_ARCHIVE_PATH),
        originalCompletion=archive["bindings"]["completion"],excludedV3=excluded,qualified=False,L=None,
        adoptedPrefixCases=185,newCases=215,rerunFromSlot=186,v3AdoptionAllowed=False)
    wrapper["epochFreeze"]=prepared_bind(root,epoch_output,epoch)
    wrapper["adoptionLedger"]=prepared_bind(root,ledger_output,ledger)
    prepared_bind(root,output,wrapper)
    return wrapper


def validate_original_case(plan, block, case, entry, paths):
    check(entry["status"]=="ok" and entry["error"] is None and len(entry["rows"])==2,
          "original whole case not accepted")
    check(entry["beforePower"]["source"]==entry["afterPower"]["source"]=="AC Power" and
          entry["beforePower"]["error"] is None and entry["afterPower"]["error"] is None,"old power evidence invalid")
    run=entry["launch"]
    check(run["exitCode"]==0 and run["terminationConfirmed"] is True and run["launchError"] is None,
          "old process was not terminal")
    result=screen.validate_case(plan,block,case,paths)
    check(not result["errors"] and result["rows"]==entry["rows"] and result["processId"]==entry["actualProbePid"],
          "original source/row/readback differs")
    return result


def preflight(root, wrapper_path):
    root=Path(root).resolve(strict=True)
    wrapper_raw=common.bounded_bytes(wrapper_path,common.MAX_JSON)
    wrapper=common.parse_json(wrapper_raw)
    check(wrapper["schemaVersion"]==1 and wrapper["kind"]=="prospective-pretranscription-continuation-v1",
          "wrong continuation protocol")
    cid=wrapper["continuationId"];check(str(uuid.UUID(cid))==cid,"invalid continuation UUID")
    check(wrapper["adoptedPrefixCases"]==185 and wrapper["newCases"]==215 and wrapper["rerunFromSlot"]==186
          and wrapper["v3AdoptionAllowed"] is False and wrapper["qualified"] is False and wrapper["L"] is None,
          "continuation policy differs")
    fingerprints={}
    wrapper_binding=dict(path=str(Path(wrapper_path).resolve().relative_to(root)),bytes=len(wrapper_raw),
                         sha256=common.sha256(wrapper_raw),mode=oct(stat.S_IMODE(Path(wrapper_path).stat().st_mode)))
    common.verify_binding(root,wrapper_binding,fingerprints,common.MAX_JSON)
    verify_excluded_epoch(root,wrapper["excludedV3"],fingerprints)
    def verify(binding,maximum=common.MAX_JSON,read=True):
        return common.verify_binding(root,binding,fingerprints,maximum,read)
    check(wrapper["originalFreeze"]["sha256"]==OLD_FREEZE_SHA and wrapper["originalArchive"]["sha256"]==OLD_ARCHIVE_SHA,
          "history source is not immutable v2")
    old=common.parse_json(verify(wrapper["originalFreeze"]))
    archive=common.parse_json(verify(wrapper["originalArchive"]))
    check(wrapper["originalCompletion"]==archive["bindings"]["completion"],"original global completion binding differs")
    original_global=common.parse_json(verify(wrapper["originalCompletion"]))
    ledger_raw=verify(wrapper["adoptionLedger"]);ledger=common.parse_json(ledger_raw)
    expected=build_ledger(old,archive);expected["continuationId"]=cid
    check(ledger==expected,"adoption ledger is not exactly canonical whole prefix185")
    epoch_raw=verify(wrapper["epochFreeze"]);epoch=common.parse_json(epoch_raw)
    common_assets(old,epoch)
    check(epoch["sourceRevision"]==wrapper["sourceRevision"] and len(epoch["sources"])==152 and
          set(EXTRA_SOURCES).issubset({s["path"] for s in epoch["sources"]}),"continuation source binding missing")
    check(epoch["outputDirectory"]!=old["outputDirectory"] and not epoch["outputDirectory"].endswith("/v3"),
          "continuation namespace must be distinct from both old runs")
    plan=screen.preflight(common.relative_path(root,wrapper["epochFreeze"]["path"]),root)
    fingerprints.update(plan["fingerprints"])
    # Do not compare changed current docs with old hashes. Only immutable old
    # source objects may establish historical source provenance.
    for source in old["sources"]:
        committed=subprocess.check_output(["git","show",OLD_REVISION+":"+source["path"]],cwd=root)
        check(len(committed)==source["bytes"] and common.sha256(committed)==source["sha256"],"historical source differs")
    compiled={}
    for p in plan["parameters"].values():compiled.update(common.compiled_sources(p))
    check(len(compiled)==35,"shared35 runtime pins required")
    old_source={s["path"]:s["sha256"] for s in old["sources"]}
    check(all(old_source.get(k)==v for k,v in compiled.items()),"old/new compiled runtime mismatch")
    old_plan=dict(root=root,sourceRevision=OLD_REVISION,parameters=plan["parameters"],
                  parameterBindings=old["parameters"],model=plan["model"])
    old_cases=slots(old)
    globals_flat=[e for b in original_global["blocks"] for e in b["cases"]]
    check(len(globals_flat)==400 and original_global["completed"] is False,"old execution history was altered")
    selected={c["id"]:c for c in common.read_json(root/old["selection"]["path"],common.MAX_JSON)["cases"]}
    adoption=[]
    for ref in ledger["entries"]:
        ordinal=ref["slot"];block,index,old_case=old_cases[ordinal-1]
        check((ref["block"],ref["caseIndex"],ref["id"])==(block["name"],index,old_case["id"]),"old identity mismatch")
        actual=verify_ledger_files(root,ref,fingerprints)
        check(actual==globals_flat[ordinal-1],"old case/global completion mismatch")
        paths={k:common.relative_path(root,old_case[k]) for k in ("output","metadata","log","completion")}
        for refraw,key in zip(ref["rawFiles"],("output","metadata","log")):
            check(refraw["path"]==old_case[key] and refraw["mode"]=="0o600","old raw path/mode mismatch")
        check(len(ref["rawFiles"])==len(actual["rawFiles"])==3,"incomplete old raw coverage")
        for raw,reference in zip(actual["rawFiles"],ref["rawFiles"]):
            check(all(raw[k]==reference[k] for k in ("sha256","bytes","mode")) and
                  Path(raw["path"])==root/reference["path"],"old completion raw hash differs")
        case=dict(selected[old_case["id"]],**{k:v for k,v in old_case.items() if k!="id"})
        validate_original_case(old_plan,block,case,actual,paths)
        adoption.append(actual)
    common.check_unchanged(dict(fingerprints=fingerprints))
    plan.update(adoption=adoption,adoptionLedger=ledger,continuationId=cid,oldSourceRevision=OLD_REVISION,
        oldFreezeSha256=OLD_FREEZE_SHA,wrapperBytes=wrapper_raw,epochBytes=epoch_raw,ledgerBytes=ledger_raw,
        wrapperSha256=common.sha256(wrapper_raw),adoptionLedgerSha256=common.sha256(ledger_raw),
        fingerprints=fingerprints,excludedV3=wrapper["excludedV3"])
    return plan


def verify_ledger_files(root, reference, fingerprints):
    raw=common.verify_binding(root,reference["completion"],fingerprints,common.MAX_JSON,True)
    for item,index in zip(reference["rawFiles"],range(3)):
        common.verify_binding(root,item,fingerprints,common.MAX_METADATA if index==1 else common.MAX_TEXT)
    check(len(reference["rawFiles"])==3,"three original raw bindings required")
    return common.parse_json(raw)


def verify_excluded_epoch(root, excluded, fingerprints):
    observed={}
    for key in ("freeze","launch","completion"):
        raw=common.verify_binding(root,excluded[key],fingerprints,common.MAX_JSON,True)
        observed[key]=common.parse_json(raw)
    frozen,launch,done=(observed[k] for k in ("freeze","launch","completion"))
    check(frozen["runId"]=="p06-pretranscription-screening-v3" and
          launch["freezeSha256"]==done["freezeSha256"]==excluded["freeze"]["sha256"],
          "excluded v3 epoch binding differs")
    check(done["powerAssertionEnded"] is True and len(done["blocks"])==4 and
          sum(len(b["cases"]) for b in done["blocks"])==400,"excluded v3 cleanup incomplete")
    pids=set();groups=set();unconfirmed=False
    for key in ("runnerPid","caffeinatePid"):
        value=launch.get(key)
        if value is not None:
            check(type(value) is int and value>0,"invalid prior owner PID")
            pids.add(value)
    for block in done["blocks"]:
        for case in block["cases"]:
            if "launch" in case:
                prior=case["launch"]
                unconfirmed=unconfirmed or prior["terminationConfirmed"] is not True
                wrapper=prior.get("wrapperPid")
                check(type(wrapper) is int and wrapper>0,"missing prior owned wrapper/group")
                pids.add(wrapper);groups.add(wrapper)
                probe=case.get("actualProbePid")
                if probe is not None:
                    check(type(probe) is int and probe>0,"invalid prior probe PID")
                    pids.add(probe)
    if unconfirmed or "ownerClearance" in excluded:
        check("ownerClearance" in excluded,"excluded v3 process end unconfirmed")
        clearance=common.parse_json(common.verify_binding(root,excluded["ownerClearance"],fingerprints,common.MAX_JSON,True))
        check(clearance["schemaVersion"]==1 and clearance["kind"]=="p06-v3-post-stop-owner-clearance" and
              clearance["authority"]=="Root independent read-only OS observation" and
              isinstance(clearance["observedUtcISO"],str) and clearance["observedUtcISO"].endswith("Z"),
              "independent Root clearance missing")
        check(clearance["v3CompletionSha256"]==excluded["completion"]["sha256"] and
              clearance["v3LaunchSha256"]==excluded["launch"]["sha256"],"clearance original binding differs")
        check(clearance["knownPids"]==sorted(pids) and clearance["processGroupIds"]==sorted(groups) and
              all(type(x) is int for x in clearance["knownPids"]+clearance["processGroupIds"]) and
              clearance["allKnownPidsAbsent"] is True and clearance["allOwnedGroupsAbsent"] is True,
              "clearance inventory/absence differs")
        check(all(definitely_absent(pid,False) for pid in sorted(pids)) and
              all(definitely_absent(group,True) for group in sorted(groups)),
              "prior owner/group is present or absence unproven")
        observed["ownerClearance"]=clearance
    return observed


def definitely_absent(identity, group):
    """Read-only zero signal: only ESRCH proves absence; never terminates."""
    try:
        if group:os.killpg(identity,0)
        else:os.kill(identity,0)
    except ProcessLookupError:
        return True
    except OSError:
        return False
    return False


def main(argv=None):
    import argparse
    import signal
    import sys
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest="action",required=True)
    creation=sub.add_parser("prepare")
    creation.add_argument("--root",type=Path,default=Path(__file__).resolve().parents[1])
    creation.add_argument("--template",required=True)
    creation.add_argument("--revision",required=True)
    creation.add_argument("--id",required=True)
    creation.add_argument("--run-directory",required=True)
    creation.add_argument("--epoch-output",required=True)
    creation.add_argument("--ledger-output",required=True)
    creation.add_argument("--output",required=True)
    creation.add_argument("--owner-clearance")
    for action in ("preflight","execute"):
        child=sub.add_parser(action);child.add_argument("freeze",type=Path)
        child.add_argument("--root",type=Path,default=Path(__file__).resolve().parents[1])
    args=parser.parse_args(argv)
    try:
        if args.action=="prepare":
            value=prepare(args.root,args.template,args.revision,args.id,args.run_directory,
                          args.epoch_output,args.ledger_output,args.output,args.owner_clearance)
            print(json.dumps(dict(prepared=True,continuationId=value["continuationId"],adopted=185,newCases=215,modelsStarted=False)))
            return 0
        plan=preflight(args.root,args.freeze)
        if args.action=="preflight":
            print(json.dumps(dict(preflight=True,adoptedValidated=185,newCases=215,rowCount=800,
                                  executionReadiness="requires fresh AC and Root-confirmed exclusive owner barrier",qualified=False)))
            return 0
        initial=screen.power_snapshot();screen.require_ac(initial)
        signal.signal(signal.SIGINT,screen.interrupt);signal.signal(signal.SIGTERM,screen.interrupt)
        power=None;power_error=None
        try:
            power=subprocess.Popen(["/usr/bin/caffeinate","-i","-w",str(os.getpid())],
                                   stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        except OSError:
            power_error="owned idle-sleep assertion could not start"
        plan["launchObservation"]=dict(startedUtc=screen.now(),runnerPid=os.getpid(),
            caffeinatePid=power.pid if power else None,powerError=power_error,initialPower=initial,qualified=False)
        def launch(case,paths):
            if power_error or power is None or power.poll() is not None:
                return dict(exitCode=None,terminationConfirmed=True,launchError=power_error or "idle-sleep assertion ended")
            return screen.launch_case(plan,case,paths)
        try:
            report=execute(plan,launch,lambda:power is not None and screen.stop_power(power))
            return 0 if report["completed"] else 1
        finally:
            if power is not None and power.poll() is None:screen.stop_power(power)
    except (ValueError,OSError,KeyError,TypeError,AttributeError,OverflowError,subprocess.SubprocessError):
        print("continuation: invalid/unbound history, incomplete owner barrier, unavailable AC or fresh output",file=sys.stderr)
        return 2


if __name__=="__main__":
    raise SystemExit(main())
