#!/usr/bin/env python3
"""Explicit frozen FullPrefix resource workload; no latency/heap qualification."""
import importlib.util
from pathlib import Path

_SPEC=importlib.util.spec_from_file_location("resource_process",Path(__file__).with_name("run-pretranscription-screening.py"))
process=importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(process)
common=process.common
check,integer=common.check,common.integer
KIND="prospective-full-prefix-resource-v1"
SERVICE="src-tauri/src/inference_service.rs"
ANCHOR=b"    /// Controlled decoder seam for deterministic worker/lifecycle verification."
INITIALIZER=b'''    /// Developer resource instrumentation constructed on the same serial owner.
    #[cfg(feature = "measurement")]
    pub fn with_initializer<D: Decoder>(
        initialize: impl FnOnce() -> Result<D, InferenceError> + Send + 'static,
    ) -> Result<Self, ServiceError> {
        Self::spawn(initialize)
    }

'''


def validate_transfer(originals,derivatives):
    """Exactly47 logical sources:46 identical, one known monotone service addition."""
    check(set(originals)==set(derivatives) and len(originals)==47 and SERVICE in originals,
          "exactly47 historical/resource logical sources required")
    identical=0
    for name,old in originals.items():
        new=derivatives[name]
        check(type(old) is bytes and type(new) is bytes,"source bytes required")
        if name==SERVICE:
            check(old.count(ANCHOR)==1 and INITIALIZER not in old and
                  new==old.replace(ANCHOR,INITIALIZER+ANCHOR,1),
                  "service must contain exactly the reviewed measurement-only addition")
        else:
            check(new==old,"shared quality algorithm source changed")
            identical+=1
    return dict(historicalSourcesChecked=47,byteIdenticalSources=identical,
                changedSource=SERVICE,initializerSha256=common.sha256(INITIALIZER),
                qualificationInferred=False)


def validate_workload(plan,report):
    """Read the actual resource schema; full feed/TC/L/heap proof is absent."""
    return _validate_workload(plan,report)


def same(a,b):
    import json
    return json.dumps(a,sort_keys=True,separators=(",",":"),allow_nan=False)==json.dumps(b,sort_keys=True,separators=(",",":"),allow_nan=False)


def key(job):
    return tuple(job[name] for name in ("session","generation","jobId","storageStart","storageEnd","timelineStart","timelineEnd","inputFrames","language"))


def trace_jobs(trace,sid):
    check(trace["enabled"] is True and type(trace["session"]) is int and trace["session"]==sid and
          type(trace["maximumJobs"]) is int and trace["maximumJobs"]==601 and type(trace["overflowEvents"]) is int and trace["overflowEvents"]==0 and
          trace["error"] is None and trace["firstOverflowJob"] is None and isinstance(trace["jobs"],list) and len(trace["jobs"])<=601,
          "resource trace invalid")
    return trace["jobs"]


def receipt_shape(receipt,job,live,candidate):
    reuse=receipt["preparedReuse"]
    check(isinstance(reuse,dict) and all(type(reuse[k]) is int and reuse[k]==job[k] for k in ("session","generation","jobId")),"foreign cache receipt")
    outcome=reuse["outcome"];frames=reuse["preparedFrames"];digest=reuse["preparedPcmSha256"];origin=reuse["originJobId"]
    invoked,native=reuse["decoderInvoked"],reuse["nativeFullAttempted"]
    check(type(invoked) is bool and type(native) is bool and (not native or invoked),"invalid native flags")
    check((frames is None)==(digest is None),"partial preparation invented")
    if frames is not None:
        check(integer(frames,16000) and frames<=4_800_000 and isinstance(digest,list) and len(digest)==32 and all(integer(v) and v<=255 for v in digest),"invalid preparation")
    if outcome=="decoded":
        check(frames is not None and type(origin) is int and origin==job["jobId"] and invoked and native,"decoded receipt lacks native attempt")
        live=(origin,frames,digest)
    elif outcome=="hit":
        check(candidate and frames is not None and type(origin) is int and 0<origin<job["jobId"] and not invoked and not native and
              live is not None and (origin,frames,digest)==live,"hit lacks live single-entry origin")
    elif outcome=="not-admitted":
        check(frames is None and origin is None and not invoked and not native,"nonadmission invents work")
        live=None
    else:
        check(outcome in ("failed","invalid-context","revoked") and origin is None and job["workerOutcome"]!="decoded-ok","accepted failed/revoked receipt")
        live=None
    check(job["workerOutcome"]!="decoded-ok" or receipt["decoderError"] is None,"accepted decoder error")
    return live


def _validate_workload(plan,report):
    candidate=plan["mode"]=="FullPrefixCandidate";frames=plan["fixture"]["frames"]
    check(type(report["schemaVersion"]) is int and report["schemaVersion"]==1 and report["kind"]=="runtime-lifecycle-resource-workload" and
          report["experimentalMode"]==plan["mode"] and report["behaviorPassed"] is True and report["resourceQualified"] is False and
          report["setupTimingPassed"] is True and report["loadError"] is None,"resource execution failed")
    for field,value in dict(parameters=plan["parameters"],parameterDocumentSha256=plan["parameterSha256"],sourceRevision=plan["sourceRevision"],
            model=plan["model"],fixture=plan["fixture"],referenceSha256=plan["referenceSha256"],preparedFixtureFrames=frames,
            preparedFixturePcmF32LeSha256=plan["preparedSha256"]).items():
        check(same(report[field],value),"resource execution binding differs")
    host=report["host"]
    check(integer(host["processId"],1) and host["debugAssertions"] is False and host["os"]=="macos","resource host/PID differs")
    cleanup=report["cleanup"]
    check(cleanup["joined"] is True and cleanup["error"] is None and integer(cleanup["elapsedMicros"]),"owner join failed")
    observed=report["observationAfterCleanup"]
    check(observed["mode"]==plan["mode"] and observed["valid"] is True and observed["validationError"] is None and
          observed["afterJoinTraceError"] is None and observed["receiptReadError"] is None,"postjoin evidence failed")
    receipts=observed["actualDecoderReceipts"]
    check(isinstance(receipts,list) and len(receipts)<=105*602,"receipt bound exceeded")
    by_key={};ordinal=0
    for receipt in receipts:
        check(type(receipt["ordinal"]) is int and receipt["ordinal"]==ordinal+1 and receipt["pcmExact"] is True and receipt["decoderReturned"] is True,"actual call receipt invalid")
        identity=key(receipt["identity"])
        check(identity not in by_key and same(receipt["decodeContext"],list(identity[:3])),"duplicate/foreign actual scoped invocation")
        by_key[identity]=receipt;ordinal+=1
    workload=report["workload"];rows=workload["rows"]
    check(workload["behaviorPassed"] is True and workload["setupTimingPassed"] is True and workload["firstError"] is None and isinstance(rows,list) and len(rows)==105,"105 completed rows required")
    total=0;seen=set();known=set();previous_job=0;stops=cancels=handoffs=0
    for index,row in enumerate(rows):
        warm=index<5;measured=index-5
        operation="StopComplete" if warm or measured%3==0 else "CancelCapture" if measured%3==1 else "CancelDecoderEntry"
        expected_id="warmup-%03d"%(index+1) if warm else "measured-%03d"%(measured+1)
        check(row["id"]==expected_id and row["warmup"] is warm and row["operation"]==operation and row["status"]=="ok" and
              row["error"] is None and type(row["session"]) is int and row["session"]==index+1 and
              row["controllerIdleAndWrapperIdle"] is True and row["ownerFreeConfirmedByNextStart"] is (index<104),"resource row/order/handoff differs")
        handoffs+=row["ownerFreeConfirmedByNextStart"]
        stop=operation=="StopComplete";stops+=stop;cancels+=not stop
        check(row["terminal"]==("Local" if stop else "Cancelled") and row["textAbsent"] is (not stop) and row["referenceMatch"] is (True if stop else None),"Stop/Cancel text outcome differs")
        feed_frames=320 if operation=="CancelCapture" else frames
        check(type(row["feedFrames"]) is int and type(row["publicationCount"]) is int and row["feedFrames"]==feed_frames and row["publicationCount"]==(feed_frames+319)//320 and
              row["feedTimingValid"] is True and integer(row["maximumPublicationDeviationNanos"]) and row["maximumPublicationDeviationNanos"]<=10_000_000 and
              row["actualPublicationAdmissionValid"] is True,"resource feed summary invalid")
        jobs=trace_jobs(row["jobTrace"],index+1);calls=[];last_cut=0;live=None
        retired=row["jobTrace"]["retiredJob"]
        process.validate_retired(retired,index+1)
        if retired is not None:check(key(retired) in known,"retired job lacks a previously observed scoped job")
        check((len(jobs)>=2 if candidate else len(jobs)==1) if stop else True,"Stop mode not exercised")
        check(operation!="CancelCapture" or not jobs,"capture cancel acquired jobs")
        for number,job in enumerate(jobs):
            check(type(job["session"]) is int and job["session"]==index+1 and type(job["generation"]) is int and job["generation"]==1 and
                  integer(job["jobId"],1) and job["jobId"]>previous_job and job["language"]=="de","foreign/reused job identity")
            previous_job=job["jobId"]
            end=job["timelineEnd"];released=job["audioReleasedAtSubmit"]
            check(all(type(job[k]) is int for k in ("timelineStart","timelineEnd","storageStart","storageEnd","inputFrames")) and
                  job["timelineStart"]==job["storageStart"]==0 and job["storageEnd"]==job["inputFrames"]==end and 0<end<=frames and
                  type(released) is bool and (not released or end==frames) and type(job["requiredStableFrames"]) is int and job["requiredStableFrames"]==end+(0 if released else 4800) and
                  job["requiredStableFrames"]<=frames,"actual invocation/pending range or role invalid")
            check(integer(job["hostSubmitOffsetNanos"]) and integer(job["serviceAcceptedOffsetNanos"]) and
                  job["hostSubmitOffsetNanos"]<=job["serviceAcceptedOffsetNanos"] and integer(job["workerSettledOffsetNanos"]) and
                  job["serviceAcceptedOffsetNanos"]<=job["workerSettledOffsetNanos"],"accepted job lacks submission/settlement")
            before=job["workerBeforeDecodeOffsetNanos"];after=job["workerAfterDecodeOffsetNanos"]
            identity=key(job);known.add(identity)
            if before is None:
                check(after is None and identity not in by_key and job["workerOutcome"]=="cancelled-pending-no-decode","pending job invents actual call")
            else:
                check(integer(before) and integer(after) and job["serviceAcceptedOffsetNanos"]<=before<=after<=job["workerSettledOffsetNanos"] and identity in by_key,"actual call lacks ordered return/settlement")
                # Resource V2 holds this actual call until Cancel revoked its
                # token, then releases it. Service AfterDecode therefore records
                # cancelled-after-decode, even if the decoder returns an error.
                if not stop:check(operation=="CancelDecoderEntry" and job["workerOutcome"]=="cancelled-after-decode",
                                  "held actual Cancel lacks verified post-decode cancellation")
                receipt=by_key[identity]
                check(same(receipt["identity"]["audioReleasedAtSubmit"],released) and
                      same(receipt["identity"]["requiredStableFrames"],job["requiredStableFrames"]),"actual invocation role differs")
                if stop:check(same(job["preparedReuse"],receipt["preparedReuse"]),"completed receipt differs from actual invocation")
                seen.add(identity);calls.append(receipt)
                live=receipt_shape(receipt,job,live,candidate)
            if stop:
                check(job["workerOutcome"]=="decoded-ok" and job["adoption"]=="accepted" and before is not None and
                      integer(job["completionObservedOffsetNanos"]) and integer(job["resultAdoptedOffsetNanos"]) and
                      job["workerSettledOffsetNanos"]<=job["completionObservedOffsetNanos"]<=job["resultAdoptedOffsetNanos"] and
                      released is (number==len(jobs)-1) and (released or end>last_cut),"Stop revision/final adoption differs")
                last_cut=end
        if stop:check(last_cut==frames and row["fullStopCoverage"] is True,"whole final coverage absent")
        if operation=="CancelDecoderEntry":
            check(len(calls)==1 and row["heldDecoderOrdinal"]==calls[0]["ordinal"] and
                  (not candidate or calls[0]["identity"]["audioReleasedAtSubmit"] is False),"held entry cancel not exercised")
        else:check(row["heldDecoderOrdinal"] is None,"unexpected held entry")
        count=len(calls)
        check(all(type(row[k]) is int and row[k]==count for k in ("decoderStarted","decoderCompleted","decoderCompared")) and
              type(row["decoderMismatches"]) is int and row["decoderMismatches"]==0 and row["decoderError"] is None,"actual row counters differ")
        total+=count
    check(len(seen)==len(receipts) and type(workload["decoderCallsStarted"]) is int and type(workload["decoderCallsCompleted"]) is int and workload["decoderCallsStarted"]==workload["decoderCallsCompleted"]==total and
          same(observed["afterJoinTrace"],rows[-1]["jobTrace"]),"unmatched actual calls or postjoin changes")
    final=report["decoderAfterCleanup"]
    check(all(type(final[k]) is int and final[k]==total for k in ("started","completed","compared")) and type(final["mismatches"]) is int and final["mismatches"]==0 and
          final["active"] is False and final["armed"] is False and final["lastError"] is None and report["decoderDiagnosticsError"] is None,"final decoder owner retained work/error")
    checkpoints=report["checkpoints"];check(len(checkpoints)==101 and type(report["metricsErrorCount"]) is int and report["metricsErrorCount"]==0,"resource metrics incomplete")
    baseline=None;vmmap=0;rss=[]
    for index,point in enumerate(checkpoints):
        row=rows[4 if index==0 else index+4];meta=point["checkpoint"];metrics=point["metrics"];include=index in (0,100)
        check(meta["afterId"]==row["id"] and meta["status"]=="ok" and meta["controllerIdleAndWrapperIdle"] is True and meta["includeVmmap"] is include,"checkpoint order differs")
        check(integer(metrics["rssBytes"]) and metrics["rssError"] is None and metrics["vmmapError"] is None and integer(metrics["elapsedMicros"]),"metric error/missing RSS")
        if include:
            check(isinstance(metrics["vmmapSummary"],str) and bool(metrics["vmmapSummary"].strip()) and len(metrics["vmmapSummary"].encode())<=1024*1024,"missing bounded vmmap summary")
            vmmap+=1
        else:check(metrics["vmmapSummary"] is None,"unexpected vmmap checkpoint")
        rss.append(metrics["rssBytes"])
        if baseline is None:baseline=rss[-1]
        check(type(point["rssDeltaFromWarmupBytes"]) is int and point["rssDeltaFromWarmupBytes"]==rss[-1]-baseline,"RSS delta differs")
    return dict(rows=105,stopRows=stops,cancelRows=cancels,followStarts=handoffs,actualCalls=total,processId=host["processId"],
                rssPoints=101,vmmapPoints=vmmap,rssBaselineBytes=baseline,rssMinBytes=min(rss),rssMaxBytes=max(rss),rssLastBytes=rss[-1],resourceQualified=False)


MODES={"FullPrefixReference":("runtime-resource-full-prefix-reference-v1","--full-prefix-reference","--print-full-prefix-reference-parameters"),
       "FullPrefixCandidate":("runtime-resource-full-prefix-candidate-v1","--full-prefix-candidate","--print-full-prefix-candidate-parameters")}
WATCHDOG_SECONDS=36000  # Runner liveness only; supports105 x300s, never an L/quality limit.


def command_for(plan):
    return ["/usr/bin/time","-l",str(plan["binary"]),str(plan["modelPath"]),"reference",str(plan["wavPath"]),
            str(plan["referencePath"]),str(plan["parameterPath"]),str(plan["paths"]["output"]),MODES[plan["mode"]][1]]


OWN={"scripts/run-full-prefix-resource.py","tests/quality/test_full_prefix_resource.py","docs/verification/full-prefix-resource-freeze-schema.json",
     "scripts/run-paced-remainder.py","scripts/run-pretranscription-screening.py","scripts/run-runtime-resource.py","scripts/screening_quality/decision.py"}


def _load_module(path,name):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


def _private_root(root,name):
    import os
    path=common.relative_path(root,name)
    check(path.is_dir() and not path.is_symlink() and (root/"artifacts") in path.resolve(strict=True).parents,"private canonical snapshot required")
    check(not os.path.islink(path),"snapshot root symlink rejected")
    return path.resolve(strict=True)


def _immutable(root,revision,name,data):
    import re
    import subprocess
    check(isinstance(revision,str) and re.fullmatch("[0-9a-f]{40}",revision),"immutable Git revision required")
    size=subprocess.run(["git","cat-file","-s",revision+":"+name],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10,check=False)
    check(size.returncode==0 and size.stdout.strip()==str(len(data)).encode(),"immutable source size differs")
    original=subprocess.run(["git","show",revision+":"+name],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10,check=False)
    check(original.returncode==0 and original.stdout==data,"immutable source differs")


def _sources(root,revision,descriptors,fp,immutable_root):
    values={};bindings={}
    for binding in descriptors:
        name=binding["path"]
        check(name not in values,"duplicate logical source")
        data=common.verify_binding(root,binding,fp,common.MAX_JSON,True)
        _immutable(immutable_root,revision,name,data)
        values[name]=data;bindings[name]=binding
    return values,bindings


def verify_baseline(root,freeze,quality_freeze,completion,manifest,fingerprints):
    """Four already-observed original UTF8 outputs, never a reference accuracy oracle.

    The copied authority tree retains repo-relative layout. Absolute raw paths
    are resolved ONLY through the corresponding immutable frozen output/argv.
    """
    root=Path(root).resolve(strict=True)
    check(freeze["mode"] in MODES,"unknown baseline mode")
    selected=[case for case in manifest["cases"] if case["id"]==freeze["fixtureCaseId"]]
    check(len(selected)==1 and selected[0]["language"]=="de" and
          selected[0]["audio"]["sha256"]==freeze["fixture"]["sha256"] and
          same(selected[0]["audio"]["frames"],freeze["fixture"]["frames"]),"baseline fixture/language differs")
    count=0;identical=None;bindings=[]
    for frozen,observed in zip(quality_freeze["blocks"],completion["blocks"]):
        if frozen["variant"]!=freeze["mode"]:continue
        check(frozen["name"]==observed["name"] and frozen["variant"]==observed["variant"],"baseline block differs")
        cases=[c for c in frozen["cases"] if c["id"]==freeze["fixtureCaseId"]]
        entries=[c for c in observed["cases"] if c["id"]==freeze["fixtureCaseId"]]
        check(len(cases)==len(entries)==1,"mode-specific baseline case missing")
        case,entry=cases[0],entries[0]
        relative=case["output"];common.relative_path(root,relative)
        binding=dict(entry["rawFiles"][0])
        check(binding["path"] in (relative,case["command"][8]),"baseline raw authority path differs")
        binding["path"]=relative
        rows=common.parse_json(common.verify_binding(root,binding,fingerprints,common.MAX_TEXT,True))
        check(isinstance(rows,list) and len(rows)==2,"baseline requires warm and measured rows")
        for row,(rid,warm) in zip(rows,(("warmup",True),("measured-001",False))):
            check(isinstance(row,dict) and set(row)=={"id","warmup","status","text"} and row["id"]==rid and
                  row["warmup"] is warm and row["status"]=="ok" and isinstance(row["text"],str),"baseline original row invalid")
            text=row["text"].encode("utf-8")
            check(bool(text.strip()) and common.sha256(text)==freeze["reference"]["sha256"] and
                  (identical is None or identical==text),"mode baseline is not four byte-identical original outputs")
            identical=text;count+=1
        bindings.append(binding)
    check(count==4,"two complete mode-specific baseline blocks required")
    return dict(fixtureCaseId=freeze["fixtureCaseId"],originalOutputsChecked=count,resultBindings=bindings,accuracyInferred=False)


def validate_eligibility(catalogue,freeze):
    """Bound pre-existing static descriptor; never an actual owner/worker proof."""
    check(type(catalogue["schemaVersion"]) is int and catalogue["schemaVersion"]==1 and
          catalogue["kind"]=="static-confirmed-pause-input-catalogue-v1","unknown static readiness descriptor")
    records=[r for r in catalogue["records"] if r["id"]==freeze["fixtureCaseId"]]
    check(len(records)==1,"static fixture readiness missing")
    record=records[0];frames=freeze["fixture"]["frames"]
    check(record["sha256"]==freeze["fixture"]["sha256"] and same(record["frames"],frames) and
          type(record["sampleRate"]) is int and record["sampleRate"]==16000 and
          type(record["channels"]) is int and record["channels"]==1,"static readiness input differs")
    offers=record["potentialDistinctOffers"]
    check(isinstance(offers,list) and len(offers)<=601,"static readiness offers invalid")
    eligible=[o for o in offers if integer(o["cutFrame"],1) and integer(o["earliest20msPublicationEndFrame"],1) and
              o["cutFrame"]+4800<=o["earliest20msPublicationEndFrame"]<frames]
    check(bool(eligible),"fixture has no statically eligible early-prefix exercise")
    return dict(eligibleCutCount=len(eligible),actualRuntimeOfferProved=False)


def _approval_and_transfer(root,freeze,parameters,fingerprints):
    """Old strict decision is used ONLY on a real byte-identical authority tree.

    The derivative tree requires a distinct explicitly bound measurement approval.
    Neither decision.validate nor its47-current-source contract is modified.
    """
    qroot=_private_root(root,freeze["qualityAuthorityRoot"])
    rroot=_private_root(root,freeze["resourceSourceRoot"])
    check(qroot!=rroot and qroot not in rroot.parents and rroot not in qroot.parents,"authority/resource snapshots must be distinct")
    qfp={};rfp={}
    load=lambda b:common.parse_json(common.verify_binding(qroot,b,qfp,common.MAX_METADATA,True))
    decision=_load_module(root/"scripts/screening_quality/decision.py","resource_strict_quality_decision")
    decision_value=load(freeze["qualityDecision"])
    qfreeze=load(decision_value["screeningFreeze"])
    qparams={name:load(b) for name,b in qfreeze["parameters"].items()}
    reference=qparams["FullPrefixReference"];candidate=qparams["FullPrefixCandidate"]
    # This call still rehashes all47 current files in qroot and requires genuine
    # bound positive numerical+external-semantic approval; never a placeholder.
    approved=decision.validate(qroot,freeze["qualityDecision"],qfp,{"reference":reference,"candidate":candidate})
    pins=common.compiled_sources(candidate)
    check(len(pins)==47 and common.compiled_sources(reference)==pins,"historical47 source closure differs")
    historical={b["path"]:b for b in qfreeze["sources"]}
    transfer=common.parse_json(common.verify_binding(root,freeze["sourceTransfer"],fingerprints,common.MAX_JSON,True))
    check(transfer["schemaVersion"]==1 and transfer["kind"]=="full-prefix-resource-source-transfer-v1" and
          transfer["historicalRevision"]==qfreeze["sourceRevision"] and transfer["resourceRevision"]==freeze["sourceRevision"] and
          transfer["qualityDecisionSha256"]==freeze["qualityDecision"]["sha256"] and
          transfer["qualityAuthorityRoot"]==freeze["qualityAuthorityRoot"] and transfer["resourceSourceRoot"]==freeze["resourceSourceRoot"],"source transfer authority differs")
    derivatives={b["path"]:b for b in transfer["sources"]}
    check(len(derivatives)==len(transfer["sources"])==47 and set(derivatives)==set(pins),"exact47 derivative bindings required")
    old={};new={}
    for name,digest in pins.items():
        check(historical[name]["sha256"]==digest,"historical compiled pin differs")
        old[name]=common.verify_binding(qroot,historical[name],qfp,common.MAX_JSON,True)
        _immutable(root,qfreeze["sourceRevision"],name,old[name])
        new[name]=common.verify_binding(rroot,derivatives[name],rfp,common.MAX_JSON,True)
        _immutable(rroot,freeze["sourceRevision"],name,new[name])
    proof=validate_transfer(old,new)
    baseline=verify_baseline(qroot,freeze,qfreeze,load(decision_value["screeningCompletion"]),load(qfreeze["manifest"]),qfp)
    source_values,source_bindings=_sources(rroot,freeze["sourceRevision"],freeze["resourceSources"],rfp,rroot)
    compiled=common.compiled_sources(parameters)
    check(set(compiled).issubset(source_values) and all(common.sha256(source_values[name])==digest for name,digest in compiled.items()),"actual resource compiled sources differ")
    required_algorithm={SERVICE,"src-tauri/src/audio.rs","src-tauri/src/controller.rs","src-tauri/src/runtime.rs",
        "src-tauri/src/transcription.rs","src-tauri/src/transcription/prefix_cache.rs","src-tauri/src/transcription/prefix_cache/evidence.rs",
        "src-tauri/src/audio/full_prefix.rs","src-tauri/src/controller/full_prefix.rs"}
    check(required_algorithm.issubset(compiled) and all(source_values[name]==new[name] for name in set(pins)&set(compiled)),
          "actual resource algorithm pins differ from the complete source transfer")
    check(same(parameters["runtimeBaseline"]["runtime"],decision_value["runtime"]) and
          decision_value["candidateFullPrefix"]["candidate"] is True,"resource decoder policy differs from quality authority")
    approval=common.parse_json(common.verify_binding(root,freeze["resourceMeasurementApproval"],fingerprints,common.MAX_JSON,True))
    check(approval["schemaVersion"]==1 and approval["kind"]=="full-prefix-resource-measurement-approval-v1" and
          approval["measurementAllowed"] is True and approval["productionActivated"] is False and approval["resourceQualified"] is False and
          approval["qualityDecisionSha256"]==freeze["qualityDecision"]["sha256"] and
          approval["sourceTransferSha256"]==freeze["sourceTransfer"]["sha256"] and approval["sourceRevision"]==freeze["sourceRevision"] and
          approval["binarySha256"]==freeze["binary"]["sha256"] and approval["parameterSha256"]==freeze["parameters"]["sha256"] and
          approval["mode"]==freeze["mode"] and
          approval["fixtureSha256"]==freeze["fixture"]["sha256"] and approval["referenceSha256"]==freeze["reference"]["sha256"] and
          approval["preparedPcmSha256"]==freeze["preparedPcmSha256"],"separate resource measurement approval required")
    eligibility=validate_eligibility(common.parse_json(common.verify_binding(root,approval["staticEligibilityCatalogue"],fingerprints,common.MAX_JSON,True)),freeze)
    return dict(quality=approved,transfer=proof,baseline=baseline,staticEligibility=eligibility,resourceQualified=False),[(qroot,qfp),(rroot,rfp)]


def preflight(path,root):
    import io
    import os
    import stat
    import wave
    root=Path(root).resolve(strict=True);raw=common.bounded_bytes(path,common.MAX_JSON);f=common.parse_json(raw);fp={}
    required={"schemaVersion","kind","sourceRevision","launcherRevision","mode","binary","parameters","printedParameters",
              "model","vad","fixture","fixtureCaseId","reference","preparedPcmSha256","qualityAuthorityRoot","resourceSourceRoot","qualityDecision",
              "sourceTransfer","resourceMeasurementApproval","resourceSources","sources","outputDirectory","resourceQualified","L"}
    check(set(f)==required and type(f["schemaVersion"]) is int and f["schemaVersion"]==1 and f["kind"]==KIND and
          f["mode"] in MODES and f["resourceQualified"] is False and f["L"] is None,"explicit resource freeze required")
    check(Path(f["binary"]["path"]).name=="runtime_resource_probe","only known resource example supported")
    common.verify_binding(root,f["binary"],fp)
    parameters=common.parse_json(common.verify_binding(root,f["parameters"],fp,1024*1024,True))
    printed=common.parse_json(common.verify_binding(root,f["printedParameters"],fp,1024*1024,True))
    check(same(parameters,printed) and parameters["configurationId"]==MODES[f["mode"]][0] and
          parameters["experimentalResourceAdapter"]["mode"]==f["mode"] and parameters["resourceQualified"] is False and
          parameters["workload"]["warmupStopSessions"]==5 and parameters["workload"]["measuredSessions"]==100 and
          parameters["workload"]["order"]==["StopComplete","CancelCapture","CancelDecoderEntry"] and
          parameters["workload"]["language"]=="de" and parameters["workload"]["captureCancelFrames"]==320,"exact printed resource mode/count required")
    common.check(parameters["runtimeBaseline"]["build"]["debugAssertions"] is False,"release resource template required")
    sources,source_bindings=_sources(root,f["launcherRevision"],f["sources"],fp,root)
    check(OWN.issubset(sources),"executing launcher/helper closure missing")
    actual={"scripts/run-full-prefix-resource.py":Path(__file__),"scripts/run-pretranscription-screening.py":Path(process.__file__),
            "scripts/run-paced-remainder.py":Path(common.__file__),"scripts/screening_quality/decision.py":root/"scripts/screening_quality/decision.py",
            "scripts/run-runtime-resource.py":Path(__file__).with_name("run-runtime-resource.py")}
    for name,file in actual.items():
        check(common.bounded_bytes(file,common.MAX_JSON)==sources[name],"executing launcher/helper differs")
    verify_printed_template(common.relative_path(root,f["binary"]["path"]),f["mode"],parameters)
    evidence,extra=_approval_and_transfer(root,f,parameters,fp)
    model,vad=f["model"],f["vad"]
    check(model["id"]=="large-v3-turbo-q5_0" and model["sha256"]==common.MODEL_SHA and model["bytes"]==574041195 and
          vad["sha256"]==common.VAD_SHA and vad["bytes"]==885098 and Path(model["path"]).name=="ggml-large-v3-turbo-q5_0.bin" and
          Path(vad["path"]).name=="ggml-silero-v5.1.2.bin" and Path(model["path"]).parent==Path(vad["path"]).parent,"model/VAD differs")
    common.verify_binding(root,model,fp);common.verify_binding(root,vad,fp)
    check(any(m["id"]==model["id"] and m["sha256"]==model["sha256"] for m in parameters["runtimeBaseline"]["models"]) and
          parameters["runtimeBaseline"]["runtime"]["speechGate"]["model"]["sha256"]==vad["sha256"],"parameter model binding differs")
    wav=common.verify_binding(root,f["fixture"],fp,20*1024*1024,True)
    with wave.open(io.BytesIO(wav),"rb") as audio:
        frames=audio.getnframes()
        check((audio.getnchannels(),audio.getframerate(),audio.getsampwidth(),audio.getcomptype())==(1,16000,2,"NONE") and
              integer(frames,320) and frames<=4_800_000 and frames==f["fixture"]["frames"],"resource PCM16 mono16k header differs")
    # Independent exact f32LE normalization, not a hash supplied by the report.
    import struct
    with wave.open(io.BytesIO(wav),"rb") as audio:pcm=audio.readframes(frames)
    import hashlib
    digest=hashlib.sha256()
    for (sample,) in struct.iter_unpack("<h",pcm):digest.update(struct.pack("<f",sample/32768.0))
    check(len(pcm)==frames*2 and digest.hexdigest()==f["preparedPcmSha256"],"prepared fixture PCM binding differs")
    reference=common.verify_binding(root,f["reference"],fp,256*1024,True)
    check(bool(reference.decode("utf-8").strip()),"nonempty qualified mode-specific baseline required")
    output=common.relative_path(root,f["outputDirectory"])
    check(not output.is_symlink() and (root/"artifacts") in output.resolve().parents and not os.path.lexists(output),"fresh private artifacts directory required")
    paths={"output":output/"workload.json","log":output/"command.log"}
    return dict(root=root,output=output,paths=paths,binary=common.relative_path(root,f["binary"]["path"]),
        modelPath=common.relative_path(root,model["path"]),wavPath=common.relative_path(root,f["fixture"]["path"]),
        referencePath=common.relative_path(root,f["reference"]["path"]),parameterPath=common.relative_path(root,f["parameters"]["path"]),
        mode=f["mode"],sourceRevision=f["sourceRevision"],parameters=parameters,parameterSha256=f["parameters"]["sha256"],
        model={k:model[k] for k in ("id","sha256","bytes")},fixture=dict(sha256=f["fixture"]["sha256"],frames=frames,sampleRate=16000,channels=1),
        referenceSha256=f["reference"]["sha256"],preparedSha256=f["preparedPcmSha256"],fingerprints=fp,extraFingerprints=extra,
        freezeBytes=raw,freezeSha256=common.sha256(raw),approvalEvidence=evidence)


def verify_printed_template(binary,mode,parameters):
    """Bound binary's existing model-free print branch; no model path is passed."""
    import os
    import subprocess
    import tempfile
    import time
    child=None
    with tempfile.TemporaryFile() as stdout,tempfile.TemporaryFile() as stderr:
        try:
            child=subprocess.Popen([str(binary),MODES[mode][2]],stdout=stdout,stderr=stderr,start_new_session=True)
            deadline=time.monotonic()+30
            while child.poll() is None:
                check(time.monotonic()<deadline and os.fstat(stdout.fileno()).st_size<=1024*1024 and
                      os.fstat(stderr.fileno()).st_size<=1024*1024,"bounded parameter printer failed")
                time.sleep(.02)
            check(child.returncode==0 and process.group_ended(child.pid) and os.fstat(stdout.fileno()).st_size<=1024*1024 and
                  os.fstat(stderr.fileno()).st_size<=1024*1024,"parameter printer exit/group/output failed")
            stdout.seek(0)
            check(same(common.parse_json(stdout.read(1024*1024+1)),parameters),"binary's actual printed template differs")
        finally:
            if child is not None and (child.poll() is None or not process.group_ended(child.pid)):
                check(process.stop_owned(child),"parameter printer termination unconfirmed")


_ACTIVE=None
_INTERRUPTED=False


def run_owned(plan,command,log_path,watchdog=WATCHDOG_SECONDS):
    import subprocess
    import os
    import json
    global _ACTIVE
    report=dict(command=command,startedUtc=process.now(),finishedUtc=None,wrapperPid=None,exitCode=None,
                terminationConfirmed=False,watchdogExpired=False,launchError=None)
    child=None
    try:
        descriptor=os.open(log_path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        os.fchmod(descriptor,0o600)
        with os.fdopen(descriptor,"wb") as log:
            child=subprocess.Popen(command,cwd=plan["root"],stdout=log,stderr=log,
                env=dict(os.environ,EVERLAST_BENCHMARK_REVISION=plan["sourceRevision"]),start_new_session=True)
            report["wrapperPid"]=child.pid;_ACTIVE=child
            print(json.dumps(dict(runId="resource",wrapperPid=child.pid,startedUtc=report["startedUtc"])),flush=True)
            check(not _INTERRUPTED,"resource supervisor interrupted")
            import time
            deadline=time.monotonic()+watchdog
            while child.poll() is None:
                check(os.fstat(log.fileno()).st_size<=common.MAX_TEXT,"owned resource log exceeded bound")
                if time.monotonic()>=deadline:raise subprocess.TimeoutExpired(command,watchdog)
                time.sleep(.02)
            report["exitCode"]=child.wait()
            check(os.fstat(log.fileno()).st_size<=common.MAX_TEXT,"owned resource log exceeded bound")
            report["terminationConfirmed"]=process.group_ended(child.pid)
            if not report["terminationConfirmed"]:
                report["launchError"]="owned descendants remained after wrapper exit"
                report["terminationConfirmed"]=process.stop_owned(child)
    except (OSError,subprocess.TimeoutExpired,KeyboardInterrupt,ValueError) as error:
        report["watchdogExpired"]=isinstance(error,subprocess.TimeoutExpired)
        report["launchError"]="owned resource launch/observation failed: "+type(error).__name__
        report["terminationConfirmed"]=process.stop_owned(child) if child is not None else True
        report["exitCode"]=child.returncode if child is not None else None
    finally:
        _ACTIVE=None;report["finishedUtc"]=process.now()
    return report


def row_slots():
    return [dict(id="warmup-%03d"%(i+1) if i<5 else "measured-%03d"%(i-4),warmup=i<5,
                 operation="StopComplete" if i<5 or (i-5)%3==0 else "CancelCapture" if (i-5)%3==1 else "CancelDecoderEntry",
                 status="unavailable",validated=False) for i in range(105)]


def check_inputs(plan):
    common.check_unchanged(plan)
    for _,fp in plan.get("extraFingerprints",[]):common.check_unchanged(dict(fingerprints=fp))


def capture_host():
    helper=_load_module(Path(__file__).with_name("run-runtime-resource.py"),"resource_host_observer")
    commands={"cpu":["/usr/sbin/sysctl","-n","machdep.cpu.brand_string"],"memoryBytes":["/usr/sbin/sysctl","-n","hw.memsize"],
              "macOS":["/usr/bin/sw_vers","-productVersion"],"macOSBuild":["/usr/bin/sw_vers","-buildVersion"],"rustc":["rustc","--version"]}
    return {name:helper.observation(command) for name,command in commands.items()}


def execute(plan,launch=None,power=None,finish=None,host=None):
    """Exactly one fixed resource process. No retry, resumed rows or subset pass."""
    power=power or process.power_snapshot
    host=host or capture_host
    output=plan["output"];output.mkdir(mode=0o700)
    common.write_private_bytes(output/"freeze.json",plan["freezeBytes"])
    report=dict(schemaVersion=1,kind="full-prefix-resource-execution-v1",freezeSha256=plan["freezeSha256"],
        sourceRevision=plan["sourceRevision"],mode=plan["mode"],startedUtc=process.now(),finishedUtc=None,
        completed=False,resourceQualified=False,L=None,rows=row_slots(),summary=None,error=None,
        launch=None,powerBefore=None,powerAfter=None,powerAssertionEnded=None,rawFiles=[])
    failure=None;raw=None
    try:
        check_inputs(plan)
        check(not _INTERRUPTED,"resource supervisor interrupted")
        report["powerBefore"]=power();process.require_ac(report["powerBefore"])
        observed_host=host()
        check(all(v.get("error") is None and isinstance(v.get("value"),str) and v["value"] for v in observed_host.values()),"host observation incomplete")
        common.write_private(output/"host.json",dict(schemaVersion=1,recordedBeforeChildLaunchUtc=process.now(),
            observations=observed_host,powerBefore=report["powerBefore"],sourceRevision=plan["sourceRevision"],
            freezeSha256=plan["freezeSha256"],command=command_for(plan)))
        launched=(launch(plan) if launch is not None else run_owned(plan,command_for(plan),plan["paths"]["log"]))
        report["launch"]=launched
        if plan["paths"]["output"].exists():
            raw=common.read_json(plan["paths"]["output"],common.MAX_METADATA)
            if isinstance(raw,dict) and isinstance(raw.get("workload"),dict) and isinstance(raw["workload"].get("rows"),list):
                for slot,row in zip(report["rows"],raw["workload"]["rows"][:105]):
                    if isinstance(row,dict):slot["status"]=row.get("status","invalid") if row.get("status") in ("ok","failed","unavailable") else "invalid"
        check(launched.get("exitCode")==0 and launched.get("terminationConfirmed") is True and launched.get("launchError") is None and
              launched.get("watchdogExpired") is False,"resource process did not complete cleanly")
        report["summary"]=validate_workload(plan,raw)
        check(integer(launched.get("wrapperPid"),1) and report["summary"]["processId"]!=launched["wrapperPid"],"wrapper and actual probe PID must be distinct")
        report["powerAfter"]=power();process.require_ac(report["powerAfter"])
        check_inputs(plan)
    except (ValueError,OSError,KeyError,TypeError,IndexError,AttributeError,OverflowError,UnicodeError,KeyboardInterrupt):
        failure="resource launch/report/owner/source/power validation failed"
    finally:
        if report["powerAfter"] is None:
            try:report["powerAfter"]=power();process.require_ac(report["powerAfter"])
            except (ValueError,OSError,KeyError,TypeError):failure=failure or "final AC Power unavailable"
        try:ended=finish() if finish is not None else False
        except (OSError,ValueError):ended=False
        report["powerAssertionEnded"]=ended is True
        if not ended:failure=failure or "owned power assertion termination unconfirmed"
    raw=common.raw_bindings(dict(results=output/"host.json",metadata=plan["paths"]["output"],log=plan["paths"]["log"]))
    for binding,role in zip(raw,("host","workload","log")):
        binding["role"]=role
        if binding.get("error") is not None or not isinstance(binding.get("sha256"),str):failure=failure or "raw archive binding failed"
        report["rawFiles"].append(binding)
    try:
        binding=common.small_binding(output/"freeze.json");binding["role"]="freeze"
    except (ValueError,OSError):
        binding=dict(role="freeze",error="raw freeze unavailable");failure=failure or "raw archive binding failed"
    report["rawFiles"].append(binding)
    if failure is not None:report["summary"]=None
    else:
        for row in report["rows"]:row["validated"]=True
    report.update(completed=failure is None,error=failure,finishedUtc=process.now())
    common.write_private(output/"completion.json",report)
    return report


def interrupt(_signal,_frame):
    global _INTERRUPTED
    _INTERRUPTED=True
    if _ACTIVE is not None:raise KeyboardInterrupt()


def main(argv=None):
    import argparse
    import json
    import os
    import signal
    import subprocess
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("freeze",type=Path);parser.add_argument("--root",type=Path,default=Path.cwd())
    parser.add_argument("--execute",action="store_true")
    args=parser.parse_args(argv);plan=preflight(args.freeze,args.root)
    if not args.execute:
        print(json.dumps(dict(preflightPassed=True,plannedRows=105,mode=plan["mode"],resourceQualified=False)))
        return 0
    assertion=None
    try:
        process.require_ac(process.power_snapshot())
        assertion=subprocess.Popen(["/usr/bin/caffeinate","-i","-w",str(os.getpid())],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        check(assertion.poll() is None,"owned power assertion did not start")
        for sig in (signal.SIGINT,signal.SIGTERM):signal.signal(sig,interrupt)
        def power():
            snapshot=process.power_snapshot()
            if assertion.poll() is not None:snapshot=dict(snapshot,error="owned power assertion ended")
            return snapshot
        report=execute(plan,power=power,finish=lambda:process.stop_power(assertion))
        assertion=None
        return 0 if report["completed"] else 1
    finally:
        if assertion is not None:process.stop_power(assertion)


if __name__=="__main__":
    try:raise SystemExit(main())
    except (ValueError,OSError,KeyError,TypeError,AttributeError,IndexError,OverflowError,UnicodeError):
        print('{"failed":true,"error":"resource freeze or execution rejected; inspect private evidence"}')
        raise SystemExit(2)
