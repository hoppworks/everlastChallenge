import copy
import importlib.util
from pathlib import Path
import tempfile
import json
import unittest
from unittest import mock

SPEC=importlib.util.spec_from_file_location("central",Path(__file__).resolve().parents[2]/"scripts/run-pretranscription-central.py")
m=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(m)

def results_sha(rows):
    return m.common.sha256(json.dumps(rows,ensure_ascii=False,sort_keys=True,indent=2).encode())

def cases():
    return [dict(id=f"{lang}{seconds}",language=lang,reference="independent public reference",
        **{"class":"clean","split":"acceptance"},
        audio=dict(path=f"artifacts/{lang}{seconds}.wav",frames=seconds*16000,sampleRate=16000,sha256=f"{n+1:064x}"))
        for n,(lang,seconds) in enumerate((l,s) for l in ("de","en") for s in (5,10,15))]

def parameters(candidate):
    return dict(configurationId="p06-six-case-pretranscription-"+("candidate" if candidate else "reference")+"-v1",
        build=dict(debugAssertions=False),
        runtime=dict(fullParams=dict(no_timestamps=False,suppress_nst=False,audio_ctx=0),context=dict(flash_attn=False,n_audio_ctx=1500)),
        pretranscription=dict(candidate=candidate),
        preprocessing=dict(mode=("continuous stable DSP; confirmed-pause prefixes and one final rest; per-job unchanged reference energy/VAD/ASR preparation" if candidate else "matched full-recording reference; unchanged energy/VAD/ASR preparation")),
        measurement=dict(warmupRuns=1,measuredRunsPerCase=30,totalRows=181,chunkMillis=20,jitterToleranceNanos=10_000_000,
            maximumSleepNanos=1_000_000,finalSpinNanos=250_000,publisherThread=dict(policy="timeConstrained")))

def documents(candidate=False):
    cs=cases(); params=parameters(candidate)
    plan=dict(cases=cs,sourceRevision="a"*40,model=dict(id="large-v3-turbo-q5_0",sha256="b"*64),
        manifestSha256="c"*64,parameters={"reference":parameters(False),"candidate":parameters(True)},
        parameterHashes={"reference":"d"*64,"candidate":"e"*64})
    pubs={}
    req=dict(periodTicks=480000,computationTicks=12000,constraintTicks=24000,preemptible=True)
    snap=dict(timeConstraint=req,defaultPolicy=False,basicPolicy=2,cpu=dict(userNanos=0,systemNanos=0))
    publisher=dict(policy="timeConstrained",requested=req,before=snap,configured=snap,after=snap,
        beforeFeed=dict(snapshot=snap,error=None),afterFeed=dict(snapshot=snap,error=None),
        outliers=[],setupError=None,postError=None,finishError=None)
    rs=[]; runs=[]
    for i,slot in enumerate(m.slots(cs)):
        case=cs[slot["fixtureIndex"]]; frames=case["audio"]["frames"]; planned=frames*62500
        if frames not in pubs:
            pubs[frames]=[dict(startFrame=k,endFrame=min(k+320,frames),plannedNanos=min(k+320,frames)*62500,
                beforePublishNanos=min(k+320,frames)*62500+1,afterPublishNanos=min(k+320,frames)*62500+10,
                deviationNanos=10,timing="onBudget") for k in range(0,frames,320)]
        elapsed=100_000_000; stop=planned+100; terminal=stop+1_000_000_000
        job=dict(session=i+1,generation=1,language=case["language"],jobId=i+1,timelineStart=0,timelineEnd=frames,
            storageStart=0,storageEnd=frames,inputFrames=frames,audioReleasedAtSubmit=True,requiredStableFrames=frames,
            hostSubmitOffsetNanos=planned+20,serviceAcceptedOffsetNanos=planned+30,workerBeforeDecodeOffsetNanos=planned+40,
            workerAfterDecodeOffsetNanos=planned+40+elapsed,workerSettledOffsetNanos=planned+50+elapsed,
            resultAdoptedOffsetNanos=planned+60+elapsed,workerOutcome="decoded-ok",adoption="accepted")
        trace=dict(enabled=True,session=i+1,maximumJobs=601,error=None,overflowEvents=0,firstOverflowJob=None,retiredJob=None,jobs=[job])
        evidence=dict(trace=trace,error=None,inputProofs=[dict(jobId=i+1,requiredStableFrames=frames,
            publishedThroughFramesAtAdmission=frames,lastCompletePublicationOffsetNanos=planned+10)])
        row=dict(id=slot["id"],warmup=slot["warmup"],status="ok",error=None,session=i+1,expectedOutcome="Local",terminal="Local",
            controllerClockMatchesCapture=True,referenceMatch=None,qosError=None,qosEntered=False,qosFinished=False,
            publisherJoined=True,publisher=publisher,engineInputFrames=frames,engineElapsedNanos=elapsed,
            engineRtf=elapsed*16000/frames/1e9,engineRtfUnavailableReason=None,
            tStopOffsetNanos=stop,tTerminalOffsetNanos=terminal,tTextOffsetNanos=terminal,
            stopToTerminalNanos=terminal-stop,stopToTextNanos=terminal-stop,
            feed=dict(timingValid=True,plannedStopOffset=dict(secs=planned//1_000_000_000,nanos=0),
                publications=pubs[frames],waitSummary=dict(observedPublications=len(pubs[frames]),detailedPublications=0)),
            jobTrace=evidence)
        rs.append(dict(slot=slot,status="ok",text="synthetic text"))
        runs.append(dict(slot=slot,observation=row))
    variant="candidate" if candidate else "reference"
    meta=dict(schemaVersion=1,kind="paced-six-case-pretranscription-experiment",processId=123,measurementComplete=True,
        qualified=False,p05Qualified=False,qualityEvaluated=False,sourceRevision=plan["sourceRevision"],debugAssertions=False,
        manifestSha256=plan["manifestSha256"],parameterDocumentSha256=plan["parameterHashes"][variant],parameters=params,
        model=plan["model"],warmupRuns=1,measuredRunsPerCase=30,requestedRows=181,fixtures=[
            dict(id=c["id"],language=c["language"],**{"class":"clean","split":"acceptance"},
                durationSeconds=c["audio"]["frames"]//16000,audioSha256=c["audio"]["sha256"],inputFrames=c["audio"]["frames"],
                sampleRate=16000,referenceUtf8Sha256=m.common.sha256(c["reference"].encode())) for c in cs],
        contextCleanup=None,pretranscriptionCandidate=candidate,initialFailure=None,cleanup=dict(joined=True,error=None),
        runs=runs,jobTraceAfterCleanup=runs[-1]["observation"]["jobTrace"]["trace"],jobTraceAfterCleanupError=None,
        hypothesesSha256=None,statistics=None)
    return plan,rs,meta

class CentralTests(unittest.TestCase):
    def test_exact_slots_and_complete_both_modes(self):
        for candidate in (False,True):
            p,r,d=documents(candidate)
            self.assertEqual(len(r),181)
            self.assertEqual(sum(x["slot"]["warmup"] for x in r),1)
            d["statistics"]=m.statistics(p["cases"],[1_000_000_000]*180)
            d["hypothesesSha256"]=results_sha(r)
            v=m.validate_documents(p,"candidate" if candidate else "reference",r,d,results_sha(r))
            self.assertEqual(len(v["rows"]),181)
            self.assertEqual(v["statistics"]["lNanos"],1_000_000_000)
    def test_mutations_fail_closed(self):
        for mutation in ("missing","duplicate","warmup","trace","proof","settled","postjoin","policy","mode","pid","stats","sha"):
            with self.subTest(mutation=mutation):
                p,r,d=documents()
                d["statistics"]=m.statistics(p["cases"],[1_000_000_000]*180);d["hypothesesSha256"]=results_sha(r)
                if mutation=="missing":r.pop()
                if mutation=="duplicate":r[3]["slot"]=r[2]["slot"]
                if mutation=="warmup":d["runs"][0]["observation"]["warmup"]=False
                if mutation=="trace":d["runs"][100]["observation"]["jobTrace"]["trace"]["jobs"][0]["timelineStart"]=1
                if mutation=="proof":d["runs"][3]["observation"]["jobTrace"]["inputProofs"][0]["requiredStableFrames"]=0
                if mutation=="settled":d["runs"][3]["observation"]["jobTrace"]["trace"]["jobs"][0]["workerSettledOffsetNanos"]=None
                if mutation=="postjoin":d["jobTraceAfterCleanup"]={}
                if mutation=="policy":d["runs"][3]["observation"]["publisher"]=copy.deepcopy(d["runs"][3]["observation"]["publisher"]);d["runs"][3]["observation"]["publisher"]["after"]["basicPolicy"]=1
                if mutation=="mode":d["pretranscriptionCandidate"]=True
                if mutation=="pid":d["processId"]=None
                if mutation=="stats":d["statistics"]["lNanos"]=1
                if mutation=="sha":d["hypothesesSha256"]="0"*64
                with self.assertRaises((ValueError,KeyError,TypeError)):
                    m.validate_documents(p,"reference",r,d,results_sha(r))
    def test_parameter_pair_allows_only_explicit_variant_deltas(self):
        ps={"reference":parameters(False),"candidate":parameters(True)}
        m.validate_parameters(ps)
        for variant in ("reference","candidate"):
            changed=copy.deepcopy(ps);changed[variant]["runtime"]["fullParams"]["audio_ctx"]=1024
            with self.assertRaises(ValueError):m.validate_parameters(changed)
    def test_independent_pair_windows_and_owned_exclusive_lock(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/"ref-cand").mkdir()
            m.check_pair_targets(root,"cand-ref")
            with self.assertRaises(ValueError):m.check_pair_targets(root,"ref-cand")
            plan=dict(outputRoot=root,cases=cases(),freezeBytes=b"{}",freezeSha256="f"*64)
            lock=root/".active-pair"
            lock.write_text("foreign active")
            with self.assertRaises(FileExistsError):
                m.execute_pair(plan,"cand-ref",lambda *args: self.fail("must not launch"))
            self.assertEqual(lock.read_text(),"foreign active")

    def test_command_has_exact_central_argv_without_run_override_or_reference_text(self):
        plan=dict(binary=Path("/repo/target/release/examples/paced_reference_probe"),modelPath=Path("/repo/model"),
                  manifest=Path("/repo/six.json"),parameterPaths={"reference":Path("/repo/ref.json")})
        self.assertEqual(m.command_for(plan,"reference",dict(results=Path("/repo/artifacts/new.json"))),
            ["/usr/bin/time","-l","/repo/target/release/examples/paced_reference_probe",
             "/repo/model","reference","/repo/six.json","/repo/ref.json","/repo/artifacts/new.json"])

    def test_quality_decision_is_bound_to_complete_screen_and_this_candidate(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);ps={"reference":parameters(False),"candidate":parameters(True)}
            completion=dict(completed=True,blocks=[dict(cases=[dict(status="ok",rows=[
                dict(status="ok",validated=True),dict(status="ok",validated=True)]) for _ in range(100)]) for _ in range(4)])
            def bind(name,value):
                path=root/name;path.write_text(json.dumps(value));path.chmod(0o600)
                return dict(path=name,bytes=path.stat().st_size,mode="0o600",sha256=m.common.sha256(path.read_bytes()))
            frozen=bind("screen-freeze.json",{})
            quality_doc=dict(schemaVersion=1,kind="pretranscription-quality-screening-decision-v1",
                decision=dict(numericalScreenPassed=True,candidateRejected=False,
                              productionPretranscriptionEnabled=False,performanceRunStarted=False),
                qualified=False,L=None)
            quality=bind("quality.json",quality_doc)
            cb=bind("screen-completion.json",completion)
            decision=dict(kind="pretranscription-quality-decision-v1",passed=True,productionActivated=False,
                screeningFreeze=frozen,screeningCompletion=cb,qualityReport=quality,
                runtime=ps["reference"]["runtime"],candidatePretranscription=ps["candidate"]["pretranscription"])
            binding=bind("decision.json",decision)
            m.validate_quality_decision(root,binding,{},ps)
            decision["passed"]=False
            with self.assertRaises(ValueError):
                m.validate_quality_decision(root,bind("decision.json",decision),{},ps)
            decision["passed"]=True
            # The external overall/semantic approval cannot override an explicit
            # numerical rejection or an absent/unknown report schema.
            for mutation in ("numeric_false","numeric_missing","rejected","kind","schema"):
                bad_quality=copy.deepcopy(quality_doc)
                if mutation=="numeric_false":bad_quality["decision"]["numericalScreenPassed"]=False
                if mutation=="numeric_missing":del bad_quality["decision"]["numericalScreenPassed"]
                if mutation=="rejected":bad_quality["decision"]["candidateRejected"]=True
                if mutation=="kind":bad_quality["kind"]="unknown"
                if mutation=="schema":bad_quality["schemaVersion"]=2
                decision["qualityReport"]=bind("quality.json",bad_quality)
                with self.subTest(quality=mutation), self.assertRaises((ValueError,KeyError)):
                    m.validate_quality_decision(root,bind("decision.json",decision),{},ps)
            decision["qualityReport"]=bind("quality.json",quality_doc)
            completion["blocks"][3]["cases"][99]["rows"][1]["validated"]=False
            decision["screeningCompletion"]=bind("screen-completion.json",completion)
            binding=bind("decision.json",decision)
            with self.assertRaises(ValueError):m.validate_quality_decision(root,binding,{},ps)
            decision["screeningCompletion"]=cb
            # An altered bound file must not be accepted using its old SHA.
            with self.assertRaises(ValueError):m.validate_quality_decision(root,bind("decision.json",decision),{},ps)

    def test_serial_pair_success_and_report_failure_keep_all_slots(self):
        for bad in (None,"reference","candidate"):
            with self.subTest(bad=bad), tempfile.TemporaryDirectory() as d:
                plan=dict(outputRoot=Path(d),cases=cases(),freezeBytes=b"{}",freezeSha256="f"*64)
                calls=[]
                def launch(plan,variant,paths):
                    calls.append(variant)
                    for key,value in (("results",[]),("metadata",{})):
                        m.common.write_private(paths[key],value)
                    paths["log"].write_bytes(b"");paths["log"].chmod(0o600)
                    return dict(exitCode=0,terminationConfirmed=True,launchError=None,wrapperPid=999)
                def validated(plan,variant,*args):
                    if variant == bad:raise ValueError("controlled report boundary failure")
                    summary=m.statistics(plan["cases"],[1_000_000_000 if variant=="reference" else 800_000_000]*180)
                    return dict(rows=[dict(slot=s,status="ok",validated=True) for s in m.slots(plan["cases"])],
                        statistics=summary,processId=123)
                with mock.patch.object(m,"validate_documents",side_effect=validated):
                    report=m.execute_pair(plan,"ref-cand",launch)
                self.assertEqual(calls,["reference"] if bad=="reference" else ["reference","candidate"])
                self.assertEqual(sum(len(b["rows"]) for b in report["blocks"]),362)
                self.assertEqual(report["completed"],bad is None)
                self.assertFalse((Path(d)/".active-pair").exists())
                self.assertTrue((Path(d)/"ref-cand"/"completion.json").exists())
                if bad is None:self.assertAlmostEqual(report["comparison"]["improvementFraction"],.2)
                else:self.assertIsNone(report["comparison"])

    def test_raw_binding_failure_invalidates_pair_and_blocks_next_launch(self):
        for position in ("reference", "candidate"):
            with self.subTest(position=position), tempfile.TemporaryDirectory() as d:
                plan=dict(outputRoot=Path(d),cases=cases(),freezeBytes=b"{}",freezeSha256="f"*64)
                calls=[]
                def launch(plan,variant,paths):
                    calls.append(variant)
                    for key,value in (("results",[]),("metadata",{})):
                        m.common.write_private(paths[key],value)
                    paths["log"].write_bytes(b"");paths["log"].chmod(0o600)
                    return dict(exitCode=0,terminationConfirmed=True,launchError=None,wrapperPid=999)
                def checked(plan,variant,*args):
                    return dict(rows=[dict(slot=s,status="ok",validated=True) for s in m.slots(plan["cases"])],
                        statistics=m.statistics(plan["cases"],[1_000_000_000]*180),processId=123)
                original=m.common.raw_bindings
                def bindings(paths):
                    result=original(paths)
                    if position in paths["results"].name:
                        result[0]["sha256"]=None;result[0]["error"]="controlled binding failure"
                    return result
                with mock.patch.object(m,"validate_documents",side_effect=checked),mock.patch.object(m.common,"raw_bindings",side_effect=bindings):
                    report=m.execute_pair(plan,"ref-cand",launch)
                self.assertFalse(report["completed"])
                self.assertIsNone(report["comparison"])
                self.assertEqual(calls,["reference"] if position=="reference" else ["reference","candidate"])
                if position=="reference":self.assertEqual(report["blocks"][1]["status"],"unavailable")

    def test_pair_no_retry_no_fragment_resume_and_all_slots_retained(self):
        for pair,expected in (("ref-cand",["reference","candidate"]),("cand-ref",["candidate","reference"])):
            with tempfile.TemporaryDirectory() as d:
                plan=dict(outputRoot=Path(d),cases=cases(),freezeBytes=b"{}",freezeSha256="f"*64)
                calls=[]
                def launch(plan,variant,paths):
                    calls.append(variant)
                    return dict(exitCode=1,terminationConfirmed=True,launchError=None)
                report=m.execute_pair(plan,pair,launch)
                self.assertEqual(calls,expected[:1])
                self.assertFalse(report["completed"])
                self.assertEqual([x["variant"] for x in report["blocks"]],expected)
                self.assertEqual([len(x["rows"]) for x in report["blocks"]],[181,181])
                self.assertEqual(report["blocks"][1]["status"],"unavailable")
                with self.assertRaises(FileExistsError):m.execute_pair(plan,pair,launch)

if __name__=="__main__":unittest.main()
